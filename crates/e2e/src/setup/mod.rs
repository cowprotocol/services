pub mod colocation;
mod deploy;
#[macro_use]
pub mod onchain_components;
mod services;

use {
    crate::nodes::{forked_node::Forker, local_node::Resetter, TestNode, NODE_HOST},
    anyhow::{anyhow, Result},
    ethcontract::{futures::FutureExt, H160},
    shared::ethrpc::{create_test_transport, Web3},
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
pub use {deploy::*, onchain_components::*, services::*};

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

/// Repeatedly evaluate condition until it returns true or the timeout is
/// reached. If condition evaluates to true, Ok(()) is returned. If the timeout
/// is reached Err is returned.
pub async fn wait_for_condition<Fut>(
    timeout: Duration,
    mut condition: impl FnMut() -> Fut,
) -> Result<()>
where
    Fut: Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while !condition().await {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if start.elapsed() > timeout {
            return Err(anyhow!("timeout"));
        }
    }
    Ok(())
}

static NODE_MUTEX: Mutex<()> = Mutex::new(());

const DEFAULT_FILTERS: [&str; 9] = [
    "warn",
    "autopilot=debug",
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
    let mut default_filters: Vec<_> = DEFAULT_FILTERS.into_iter().map(String::from).collect();
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
    run(f, empty::<&str>(), None, None).await
}

pub async fn run_test_with_extra_filters<F, Fut, T>(
    f: F,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, None, None).await
}

pub async fn run_forked_test<F, Fut>(f: F, solver_address: H160, fork_url: String)
where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), Some(solver_address), Some(fork_url)).await
}

pub async fn run_forked_test_with_extra_filters<F, Fut, T>(
    f: F,
    solver_address: H160,
    fork_url: String,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, Some(solver_address), Some(fork_url)).await
}

async fn run<F, Fut, T>(
    f: F,
    filters: impl IntoIterator<Item = T>,
    solver_address: Option<H160>,
    fork_url: Option<String>,
) where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    observe::tracing::initialize_reentrant(&with_default_filters(filters).join(","));
    observe::panic_hook::install();

    // The mutex guarantees that no more than a test at a time is running on
    // the testing node.
    // Note that the mutex is expected to become poisoned if a test panics. This
    // is not relevant for us as we are not interested in the data stored in
    // it but rather in the locked state.
    let _lock = NODE_MUTEX.lock();

    let node = Arc::new(Mutex::new(Some(Node::new().await)));
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

/// A blockchain node for development purposes. Dropping this type will
/// terminate the node.
struct Node {
    process: Option<tokio::process::Child>,
}

impl Node {
    /// Spawn a new node instance.
    async fn new() -> Self {
        use tokio::io::AsyncBufReadExt as _;

        // Allow using some custom logic to spawn `anvil` by setting `ANVIL_COMMAND`.
        // For example if you set up a command that spins up a docker container.
        let command = std::env::var("ANVIL_COMMAND").unwrap_or("anvil".to_string());

        let mut process = tokio::process::Command::new(command)
            .arg("--port")
            .arg("8545")
            .arg("--gas-price")
            .arg("1")
            .arg("--gas-limit")
            .arg("10000000")
            .arg("--base-fee")
            .arg("0")
            .arg("--balance")
            .arg("1000000")
            .arg("--chain-id")
            .arg("1")
            .arg("--timestamp")
            .arg("1577836800")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = process.stdout.take().unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel::<String>();

        tokio::task::spawn(async move {
            let mut sender = Some(sender);
            const NEEDLE: &str = "Listening on ";
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            while let Some(line) = reader.next_line().await.unwrap() {
                tracing::trace!(line);
                if let Some(addr) = line.strip_prefix(NEEDLE) {
                    match sender.take() {
                        Some(sender) => sender.send(format!("http://{addr}")).unwrap(),
                        None => tracing::error!(addr, "detected multiple anvil endpoints"),
                    }
                }
            }
        });

        let _url = tokio::time::timeout(tokio::time::Duration::from_secs(1), receiver)
            .await
            .expect("finding anvil URL timed out")
            .unwrap();
        Self {
            process: Some(process),
        }
    }

    /// Most reliable way to kill the process. If you get the chance to manually
    /// clean up the [`Node`] do it because the [`Drop::drop`]
    /// implementation can not be as reliable due to missing async support.
    async fn kill(&mut self) {
        let mut process = match self.process.take() {
            Some(node) => node,
            // Somebody already called `Node::kill()`
            None => return,
        };

        if let Err(err) = process.kill().await {
            tracing::error!(?err, "failed to kill node process");
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let mut process = match self.process.take() {
            Some(process) => process,
            // Somebody already called `Node::kill()`
            None => return,
        };

        // This only sends SIGKILL to the process but does not wait for the process to
        // actually terminate. But since `anvil` is fairly well behaved that
        // should be good enough.
        if let Err(err) = process.start_kill() {
            tracing::error!("failed to kill anvil: {err:?}");
        }
    }
}
