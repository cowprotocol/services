use bollard::{
    container::{Config, CreateContainerOptions, ListContainersOptions},
    image::BuildImageOptions,
    service::{HostConfig, PortBinding},
};
pub mod colocation;
mod deploy;
#[macro_use]
pub mod onchain_components;
mod services;

use {
    crate::nodes::{Node, NODE_HOST},
    anyhow::{anyhow, Result},
    ethcontract::{futures::FutureExt, H160},
    futures::StreamExt,
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
    F: FnOnce(Web3, DbUrl) -> Fut,
    Fut: Future<Output = ()>,
{
    run(f, empty::<&str>(), None).await
}

pub async fn run_test_with_extra_filters<F, Fut, T>(
    f: F,
    extra_filters: impl IntoIterator<Item = T>,
) where
    F: FnOnce(Web3, DbUrl) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, None).await
}

pub async fn run_forked_test<F, Fut>(f: F, solver_address: H160, fork_url: String)
where
    F: FnOnce(Web3, DbUrl) -> Fut,
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
    F: FnOnce(Web3, DbUrl) -> Fut,
    Fut: Future<Output = ()>,
    T: AsRef<str>,
{
    run(f, extra_filters, Some((solver_address, fork_url))).await
}

#[derive(Debug, Clone)]
pub struct DbUrl(pub reqwest::Url);

const POSTGRES_IMAGE: &str = "postgres:latest";

async fn run<F, Fut, T>(f: F, filters: impl IntoIterator<Item = T>, fork: Option<(H160, String)>)
where
    F: FnOnce(Web3, DbUrl) -> Fut,
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

    // generate random container name
    let docker = bollard::Docker::connect_with_socket_defaults().unwrap();

    let postgres = docker
        .create_container::<&str, _>(
            None,
            Config {
                image: Some(POSTGRES_IMAGE),
                env: Some(vec![
                    "POSTGRES_HOST_AUTH_METHOD=trust",
                    "POSTGRES_USER=martin",
                    "POSTGRES_PASSWORD=123",
                ]),
                cmd: Some(vec!["-d", "postgres"]),
                host_config: Some(HostConfig {
                    auto_remove: Some(true),
                    publish_all_ports: Some(true),
                    // port_bindings: Some(
                    //     [(
                    //         "5432/tcp".into(),
                    //         Some(vec![PortBinding {
                    //             host_ip: Some("localhost".into()),
                    //             host_port: Some("5432".into()),
                    //         }]),
                    //     )]
                    //     .into(),
                    // ),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();


    docker
        .start_container::<&str>(&postgres.id, None)
        .await
        .unwrap();

    let summary = docker
        .list_containers(Some(ListContainersOptions {
            filters: [("id".into(), vec![postgres.id.clone()])].into(),
            ..Default::default()
        }))
        .await.unwrap();
    let db_port = summary[0].ports.as_ref().unwrap()[0].public_port.unwrap();

    let migrations = docker
        .create_container::<&str, _>(
            None,
            Config {
                image: Some("migrations"),
                cmd: Some(vec!["migrate"]),
                env: Some(vec![
                    &format!("FLYWAY_URL=jdbc:postgresql://localhost:{db_port}/?user=martin&password="),
                ]),
                network_disabled: Some(false),
                host_config: Some(HostConfig {
                    auto_remove: Some(true),
                    network_mode: Some("host".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    docker
        .start_container::<&str>(&migrations.id, None)
        .await
        .unwrap();

    // wait until migrations are done
    assert!(docker
        .wait_container::<&str>(&migrations.id, None)
        .next()
        .await
        .unwrap()
        .unwrap()
        .error
        .is_none());

    let node = match &fork {
        Some((_, fork)) => Node::forked(fork).await,
        None => Node::new().await,
    };

    let node = Arc::new(Mutex::new(Some(node)));
    let node_panic_handle = node.clone();
    observe::panic_hook::prepend_panic_handler(Box::new(move |_| {
        // Drop node in panic handler because `.catch_unwind()` does not catch all
        // panics
        let _ = node_panic_handle.lock().unwrap().take();
    }));

    let url = node.lock().unwrap().as_ref().map(|node| node.url.clone()).unwrap();
    let http = create_test_transport(url.as_str());
    let web3 = Web3::new(http);
    if let Some((solver, _)) = &fork {
        Web3::api::<crate::nodes::forked_node::ForkedNodeApi<_>>(&web3)
            .impersonate(solver)
            .await
            .unwrap();
    }

    let db_url: reqwest::Url = format!("postgres://127.0.0.1:{db_port}").parse().unwrap();
    let db_url = DbUrl(db_url);

    // Hack: the closure may actually be unwind unsafe; moreover, `catch_unwind`
    // does not catch some types of panics. In this cases, the state of the node
    // is not restored. This is not considered an issue since this function
    // is supposed to be used in a test environment.
    let result = AssertUnwindSafe(f(web3.clone(), db_url)).catch_unwind().await;

    let node = node.lock().unwrap().take();
    if let Some(mut node) = node {
        node.kill().await;
    }
    let _ = docker.kill_container::<&str>(&postgres.id, None).await;

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}
