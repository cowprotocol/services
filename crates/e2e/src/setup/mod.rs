pub mod colocation;
mod deploy;
#[macro_use]
pub mod onchain_components;
mod docker;
mod services;

use {
    anyhow::{anyhow, Result},
    docker::{ContainerRegistry, Node},
    ethcontract::H160,
    futures::FutureExt,
    shared::ethrpc::{create_test_transport, Web3},
    std::{
        future::Future,
        io::Write,
        iter::empty,
        panic::{self, AssertUnwindSafe},
        time::Duration,
    },
    tempfile::TempPath,
};
pub use {deploy::*, docker::Db, onchain_components::*, services::*};

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

pub async fn run_forked_test<F, Fut>(f: F, solver_address: H160, fork_url: String)
where
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), Some((solver_address, fork_url))).await
}

pub async fn run_forked_test_with_extra_filters<F, Fut, T>(
    f: F,
    solver_address: H160,
    fork_url: String,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, Some((solver_address, fork_url))).await
}

async fn run<F, Fut, T>(f: F, filters: impl IntoIterator<Item = T>, fork: Option<(H160, String)>)
where
    F: FnOnce(Web3, Db) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    observe::tracing::initialize_reentrant(&with_default_filters(filters).join(","));

    tracing::info!("setting up test environment");

    let registry = ContainerRegistry::default();

    let set_up_and_run = async {
        let start_db = Db::new(&registry);
        let start_node = match &fork {
            Some((_, fork)) => Node::forked(fork, &registry).boxed(),
            None => Node::new(&registry).boxed(),
        };
        let (db, node) = futures::join!(start_db, start_node);

        let http = create_test_transport(node.url.as_str());
        let web3 = Web3::new(http);

        if let Some((solver, _)) = &fork {
            Web3::api::<crate::nodes::forked_node::ForkedNodeApi<_>>(&web3)
                .impersonate(solver)
                .await
                .unwrap();
        }

        tracing::info!("test environment ready");

        // Hack: the closure may actually be unwind unsafe; moreover, `catch_unwind`
        // does not catch some types of panics. In this cases, the state of the node
        // is not restored. This is not considered an issue since this function
        // is supposed to be used in a test environment.
        AssertUnwindSafe(f(web3.clone(), db.clone()))
            .catch_unwind()
            .await
    };

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
