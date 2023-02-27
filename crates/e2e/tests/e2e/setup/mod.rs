mod deploy;
#[macro_use]
mod onchain_components;
mod services;

use {
    crate::local_node::{Resetter, NODE_HOST},
    anyhow::{anyhow, Result},
    ethcontract::futures::FutureExt,
    shared::ethrpc::{create_test_transport, Web3},
    std::{
        future::Future,
        io::Write,
        panic::{self, AssertUnwindSafe},
        sync::Mutex,
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
        tokio::time::sleep(Duration::from_millis(100)).await;
        if start.elapsed() > timeout {
            return Err(anyhow!("timeout"));
        }
    }
    Ok(())
}

static NODE_MUTEX: Mutex<()> = Mutex::new(());

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
    let filters = [
        "e2e=debug",
        "orderbook=debug",
        "solver=debug",
        "autopilot=debug",
        "orderbook::api::request_summary=off",
    ]
    .join(",");

    shared::tracing::initialize_reentrant(&filters);
    shared::exit_process_on_panic::set_panic_hook();

    // The mutex guarantees that no more than a test at a time is running on
    // the testing node.
    // Note that the mutex is expected to become poisoned if a test panics. This
    // is not relevant for us as we are not interested in the data stored in
    // it but rather in the locked state.
    let _lock = NODE_MUTEX.lock();

    let http = create_test_transport(NODE_HOST);
    let web3 = Web3::new(http);
    let resetter = Resetter::new(&web3).await;

    // Hack: the closure may actually be unwind unsafe; moreover, `catch_unwind`
    // does not catch some types of panics. In this cases, the state of the node
    // is not restored. This is not considered an issue since this function
    // is supposed to be used in a test environment.
    let result = AssertUnwindSafe(f(web3.clone())).catch_unwind().await;

    resetter.reset().await;
    services::clear_database().await;

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}
