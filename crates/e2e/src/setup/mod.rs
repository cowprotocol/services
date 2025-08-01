pub mod colocation;
mod deploy;
#[macro_use]
pub mod onchain_components;
pub mod fee;
mod services;
mod solver;

use {
    crate::nodes::{NODE_HOST, Node},
    anyhow::{Result, anyhow},
    ethcontract::futures::FutureExt,
    shared::ethrpc::{Web3, create_test_transport},
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
    "cow_amm=debug",
    "driver=debug",
    "e2e=debug",
    "orderbook=debug",
    "shared=debug",
    "solver=debug",
    "solvers=debug",
    "orderbook::api::request_summary=off",
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
/// This function also intializes tracing and sets panic hook.
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
    observe::tracing::initialize_reentrant(&obs_config);
    observe::panic_hook::install();

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
        // Drop node in panic handler because `.catch_unwind()` does not catch all
        // panics
        let _ = node_panic_handle.lock().unwrap().take();
    }));

    let http = create_test_transport(NODE_HOST);
    let web3 = Web3::new(http);

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

#[macro_export]
macro_rules! assert_approximately_eq {
    ($executed_value:expr_2021, $expected_value:expr_2021) => {{
        let lower = $expected_value * U256::from(99999999999u128) / U256::from(100000000000u128);
        let upper =
            ($expected_value * U256::from(100000000001u128) / U256::from(100000000000u128)) + 1;
        assert!(
            $executed_value >= lower && $executed_value <= upper,
            "Expected: ~{}, got: {}",
            $expected_value,
            $executed_value
        );
    }};
}
