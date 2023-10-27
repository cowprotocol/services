pub mod colocation;
mod deploy;
#[macro_use]
pub mod onchain_components;
mod db;
mod services;

use {
    crate::nodes::Node,
    anyhow::{anyhow, Result},
    ethcontract::H160,
    futures::FutureExt,
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
pub use {db::Db, deploy::*, onchain_components::*, services::*};

/// Create a temporary file with the given content.
pub fn config_tmp_file<C: AsRef<[u8]>>(content: C) -> TempPath {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(content.as_ref()).unwrap();
    file.into_temp_path()
}

// TODO wrap DB in constructor
// have fn to await migrations container
// remove container on drop
// remove containers on ctrl-c and panic
// figure out how to pass around DB URL
// rewrite anvil to use a container as well

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
    observe::panic_hook::install();

    tracing::info!("setting up test environment");

    let db = Db::new().await;
    let node = match &fork {
        Some((_, fork)) => Node::forked(fork).await,
        None => Node::new().await,
    };

    // Idea: write a function that spawns a blocking task that cleans up all the
    // containers. This can be called at the end of the function or in a
    // panic/signal handler.

    let node = Arc::new(Mutex::new(Some(node)));
    let node_panic_handle = node.clone();
    observe::panic_hook::prepend_panic_handler(Box::new(move |_| {
        // Drop node in panic handler because `.catch_unwind()` does not catch all
        // panics
        let _ = node_panic_handle.lock().unwrap().take();
    }));

    let url = node
        .lock()
        .unwrap()
        .as_ref()
        .map(|node| node.url.clone())
        .unwrap();
    let http = create_test_transport(url.as_str());
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
    let result = AssertUnwindSafe(f(web3.clone(), db)).catch_unwind().await;

    let node = node.lock().unwrap().take();
    if let Some(mut node) = node {
        node.kill().await;
    }

    // db.kill().await;

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}
