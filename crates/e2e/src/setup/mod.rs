pub mod colocation;
mod deploy;
pub mod onchain_components;
pub mod proxy;
mod services;
mod solver;

use {
    crate::nodes::{NODE_HOST, Node},
    ::alloy::signers::local::PrivateKeySigner,
    anyhow::{Result, anyhow},
    ethrpc::{Web3, alloy::MutWallet},
    futures::FutureExt,
    std::{
        future::Future,
        io::Write,
        iter::empty,
        panic::{self, AssertUnwindSafe},
        sync::{Arc, Mutex},
        time::Duration,
    },
    tempfile::TempPath,
};
pub use {deploy::*, onchain_components::*, services::*, solver::*};

/// Create a temporary file with the given content.
pub fn config_tmp_file<C: AsRef<[u8]>>(content: C) -> TempPath {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(content.as_ref()).unwrap();
    file.into_temp_path()
}

/// Reasonable default timeout for `wait_for_condition`.
///
/// The correct timeout depends on the condition and where the test is run. For
/// example, it can take a couple of seconds for a newly placed order to show up
/// in the auction. When running on Github CI, anything can take an unexpectedly
/// long time.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// Repeatedly evaluates condition until it returns a truthy value
/// (true, Some(true), Result(true)) or the timeout is reached.
/// If condition evaluates to truthy, Ok(()) is returned. If the timeout
/// is reached Err is returned.
pub async fn wait_for_condition<Fut>(
    timeout: Duration,
    mut condition: impl FnMut() -> Fut,
) -> Result<()>
where
    Fut: Future<Output: AwaitableCondition>,
{
    let start = std::time::Instant::now();
    while !condition().await.was_successful() {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if start.elapsed() > timeout {
            return Err(anyhow!("timeout"));
        }
    }
    Ok(())
}

pub trait AwaitableCondition {
    fn was_successful(&self) -> bool;
}

impl AwaitableCondition for bool {
    fn was_successful(&self) -> bool {
        *self
    }
}

impl AwaitableCondition for Option<bool> {
    fn was_successful(&self) -> bool {
        self.is_some_and(|inner| inner)
    }
}

impl AwaitableCondition for Result<bool> {
    fn was_successful(&self) -> bool {
        self.as_ref().is_ok_and(|inner| *inner)
    }
}

static NODE_MUTEX: Mutex<()> = Mutex::new(());

const DEFAULT_FILTERS: &[&str] = &[
    "warn",
    "autopilot=debug",
    "driver=debug",
    "e2e=debug",
    "orderbook=debug",
    "shared=debug",
    "solver=debug",
    "solvers=debug",
    "orderbook::api::request_summary=off",
    "simulator=debug",
    "price_estimation=debug",
];

fn with_default_filters<T>(custom_filters: impl IntoIterator<Item = T>) -> Vec<String>
where
    T: AsRef<str>,
{
    let mut default_filters: Vec<_> = DEFAULT_FILTERS.iter().map(|s| s.to_string()).collect();
    default_filters.extend(custom_filters.into_iter().map(|f| f.as_ref().to_owned()));

    default_filters
}

/// *Testing* function that takes a closure and runs it on a local testing node
/// and database. Before each test, it creates a snapshot of the current state
/// of the chain. The saved state is restored at the end of the test.
/// The database is cleaned at the end of the test.
///
/// This function also initializes tracing and sets panic hook.
///
/// Note that tests calling with this function will not be run simultaneously.
pub async fn run_test<F, Fut>(f: F)
where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), None).await
}

pub async fn run_test_with_extra_filters<F, Fut, T>(
    f: F,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, None).await
}

pub async fn run_forked_test<F, Fut>(f: F, fork_url: String)
where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), Some((fork_url, None))).await
}

pub async fn run_forked_test_with_block_number<F, Fut>(f: F, fork_url: String, block_number: u64)
where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), Some((fork_url, Some(block_number)))).await
}

pub async fn run_forked_test_with_extra_filters<F, Fut, T>(
    f: F,
    fork_url: String,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, Some((fork_url, None))).await
}

pub async fn run_forked_test_with_extra_filters_and_block_number<F, Fut, T>(
    f: F,
    fork_url: String,
    block_number: u64,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, Some((fork_url, Some(block_number)))).await
}

async fn run<F, Fut, T>(
    f: F,
    filters: impl IntoIterator<Item = T>,
    fork: Option<(String, Option<u64>)>,
) where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    let obs_config = observe::Config::new(
        &with_default_filters(filters).join(","),
        Some(tracing::Level::ERROR),
        false,
        None,
    );
    observe::tracing::init::initialize_reentrant(&obs_config);
    observe::panic_hook::install();

    services::ensure_e2e_readonly_user().await;
    // The mutex guarantees that no more than a test at a time is running on
    // the testing node.
    // Note that the mutex is expected to become poisoned if a test panics. This
    // is not relevant for us as we are not interested in the data stored in
    // it but rather in the locked state.
    let _lock = NODE_MUTEX.lock();

    let node = match fork {
        Some((fork, block_number)) => Node::forked(fork, block_number).await,
        None => Node::new().await,
    };

    let node = Arc::new(Mutex::new(Some(node)));
    let node_panic_handle = node.clone();
    observe::panic_hook::prepend_panic_handler(Box::new(move |_| {
        // Drop node in panic handler because `.catch_unwind()` does not catch
        // all panics
        let _ = node_panic_handle.lock().unwrap().take();
    }));

    let web3 = Web3::new_from_url(NODE_HOST);
    register_anvil_accounts(&web3.wallet);

    services::clear_database().await;
    // Hack: the closure may actually be unwind unsafe; moreover, `catch_unwind`
    // does not catch some types of panics. In this cases, the state of the node
    // is not restored. This is not considered an issue since this function
    // is supposed to be used in a test environment.
    let result = AssertUnwindSafe(f(web3.clone())).catch_unwind().await;

    let node = node.lock().unwrap().take();
    if let Some(mut node) = node {
        node.kill().await;
    }
    services::clear_database().await;

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}

/// Registers accounts pre-funded by anvil in the wallet.
fn register_anvil_accounts(wallet: &MutWallet) {
    // Keys derived from the standard anvil test mnemonic
    // ("test test test test test test test test test test test junk"),
    //
    // NEVER USE ANY OF THESE KEYS TO HOLD TOKENS ON REAL CHAINS.
    // THE TOKENS WILL GET STOLEN!
    const TEST_PRIVATE_KEYS: [&str; 10] = [
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
        "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
        "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6",
        "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
        "0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba",
        "0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e",
        "0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356",
        "0xdbda1821b80551c9d65939329250298aa3472ba22feea921c0cf5d620ea67b97",
        "0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6",
    ];
    for key in TEST_PRIVATE_KEYS {
        let signer: PrivateKeySigner = key.parse().unwrap();
        wallet.register_signer(signer);
    }
}

#[macro_export]
macro_rules! assert_approximately_eq {
    ($executed_value:expr_2021, $expected_value:expr_2021) => {{
        let lower = $expected_value * ::alloy::primitives::U256::from(99999999999u128)
            / ::alloy::primitives::U256::from(100000000000u128);
        let upper = ($expected_value * ::alloy::primitives::U256::from(100000000001u128)
            / ::alloy::primitives::U256::from(100000000000u128))
            + ::alloy::primitives::U256::ONE;
        assert!(
            $executed_value >= lower && $executed_value <= upper,
            "Expected: ~{}, got: {}, ({lower}, {upper})",
            $expected_value,
            $executed_value
        );
    }};
}
