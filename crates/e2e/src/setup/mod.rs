pub mod colocation;
mod deploy;
#[macro_use]
pub mod onchain_components;
mod services;

use {
    anyhow::{anyhow, Result},
    docker::{ContainerRegistry, Node},
    ethrpc::Web3Transport,
    futures::FutureExt,
    std::{
        future::Future,
        io::Write,
        iter::empty,
        panic::{self, AssertUnwindSafe},
        time::Duration,
    },
    tempfile::TempPath,
};
pub use {deploy::*, docker::db::Db, onchain_components::*, services::*};

/// Component containing all ethereum RPC relevant data for testing purposes.
#[derive(Clone, Debug)]
pub struct Web3 {
    /// The client that's used to actually communicate with the node.
    pub client: ethrpc::Web3,
    /// We only expose the port because the host is always 127.0.0.1 and the
    /// caller might want to use different schemes like http or websockets.
    pub port: u16,
}

/// Create a temporary file with the given content.
fn config_tmp_file<C: AsRef<[u8]>>(content: C) -> TempPath {
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
    "hyper=trace",
    "reqwest=trace",
];

fn with_default_filters<T>(custom_filters: impl IntoIterator<Item = T>) -> Vec<String>
where
    T: AsRef<str>,
{
    let mut default_filters: Vec<_> = DEFAULT_FILTERS.iter().map(|f| String::from(*f)).collect();
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
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), None).await
}

pub async fn run_test_with_extra_filters<F, Fut, T>(
    f: F,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, None).await
}

pub async fn run_forked_test<F, Fut>(f: F, fork_url: String)
where
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), Some((fork_url, None))).await
}

pub async fn run_forked_test_with_block_number<F, Fut>(f: F, fork_url: String, block_number: u64)
where
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), Some((fork_url, Some(block_number)))).await
}

pub async fn run_forked_test_with_extra_filters<F, Fut, T>(
    f: F,
    fork_url: String,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3, Db) -> Fut,
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
    F: FnOnce(Web3, Db) -> Fut,
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
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    observe::tracing::initialize_reentrant(&with_default_filters(filters).join(","));

    tracing::info!("setting up test environment");

    let registry = ContainerRegistry::default();

    let set_up_and_run = async {
        let start_db = Db::new(&registry);
        let start_node = match fork {
            Some((fork, block_number)) => Node::forked(fork, &registry, block_number).boxed(),
            None => Node::new(&registry).boxed(),
        };
        let (db, node) = futures::join!(start_db, start_node);

        let transport = Web3Transport::new(
            web3::transports::WebSocket::new(&format!("ws://127.0.0.1:{}", node.port))
                .await
                .unwrap(),
        );

        let web3 = Web3 {
            client: ethrpc::Web3::new(transport),
            port: node.port,
        };

        tracing::info!("test environment ready; begin test");

        f(web3.clone(), db.clone()).await
    };

    // Hack: the closure may actually be unwind unsafe; moreover, `catch_unwind`
    // does not catch some types of panics. In this cases, the state of the node
    // is not restored. This is not considered an issue since this function
    // is supposed to be used in a test environment.
    let set_up_and_run = AssertUnwindSafe(set_up_and_run).catch_unwind();
    futures::pin_mut!(set_up_and_run);

    tokio::select! {
        result = &mut set_up_and_run => {
            registry.kill_all().await;
            if let Err(err) = result {
                panic::resume_unwind(err);
            }
        },
        _ = shutdown_signal() => {
            tracing::error!("test aborted");
            registry.kill_all().await;
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept main signals for graceful shutdown
    // Kubernetes sends sigterm, whereas locally sigint (ctrl-c) is most common
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await
    };
    let sigint = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .unwrap()
            .recv()
            .await;
    };
    futures::pin_mut!(sigint);
    futures::pin_mut!(sigterm);
    futures::future::select(sigterm, sigint).await;
}

#[cfg(windows)]
async fn shutdown_signal() {
    // We don't support signal handling on windows
    std::future::pending().await
}
