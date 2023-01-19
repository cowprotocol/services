//! `dev-geth` is a small program to help with running [geth] in dev mode to
//! serve as an environment for automated tests. Geth doesn't provide a way to
//! reset the dev environment, other than restarting the geth program itself.
//! `dev-geth` exposes an HTTP port to which a POST request can be made in order
//! to restart geth and provide a clean testing environment. Note that
//! restarting the program will wait until geth starts responding to RPC
//! requests before returning 200 to the caller.
//!
//! geth is the only node with dev mode which implements the
//! `eth_createAccessList` endpoint. When hardhat adds support for
//! `eth_createAccessList`, this can be removed from our infrastructure.
//!
//! NOTE: This program should only be ran from docker via Dockerfile.dev-geth!
//! It makes assumptions about its environment.
//!
//! [geth]: https://geth.ethereum.org/

use {
    dashmap::DashMap,
    std::{process::Stdio, sync::Arc},
};

const PORT: &str = "8547";
const GETH_PROGRAM: &str = "geth";
const GETH_ARGS: [&str; 6] = [
    "--dev",
    "--http",
    "--http.addr",
    "0.0.0.0",
    "--http.api",
    "web3,eth,net,debug",
];

/// The base datadir used to ensure that the genesis block is always the same.
const BASE_DATADIR: &str = "/base-datadir";

#[tokio::main]
async fn main() {
    run().await
}

// Avoid the IDE issues caused by #[tokio::main].
async fn run() {
    let router: axum::Router<State> = axum::Router::new();
    let router = router.route("/", axum::routing::post(start));
    let router = router.route("/:port", axum::routing::delete(stop));
    let router = router.with_state(State(Arc::new(Default::default())));

    axum::Server::bind(&format!("0.0.0.0:{PORT}").parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Port(String);

impl Port {
    fn free() -> Self {
        Self(
            std::net::TcpListener::bind("0.0.0.0:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
                .to_string(),
        )
    }
}

#[derive(Debug, Default)]
struct Processes(DashMap<Port, tokio::process::Child>);

impl Processes {
    async fn start(&self) -> Port {
        let port = Port::free();
        let datadir = format!("/{}", port.0);
        let status = tokio::process::Command::new("cp")
            .arg("-r")
            .arg(BASE_DATADIR)
            .arg(&datadir)
            .spawn()
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert!(status.success());
        let process = tokio::process::Command::new(GETH_PROGRAM)
            .args(GETH_ARGS.into_iter().chain([
                "--http.port",
                &port.0,
                "--datadir",
                &datadir,
                "--authrpc.port",
                &Port::free().0,
            ]))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        Self::wait_for_geth(&port).await;
        self.0.insert(port.clone(), process);
        port
    }

    async fn stop(&self, port: Port) {
        let (_, mut process) = self
            .0
            .remove(&port)
            .expect("no process running at given port");
        tokio::fs::remove_dir_all(format!("/{}", port.0))
            .await
            .expect("failed to delete the datadir");
        process.kill().await.unwrap();
    }

    async fn wait_for_geth(port: &Port) {
        let web3 = web3::Web3::new(
            web3::transports::Http::new(&format!("http://localhost:{}", port.0))
                .expect("valid URL"),
        );
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                if web3.eth().accounts().await.is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("timed out while waiting for geth to become reachable");
    }
}

async fn start(axum::extract::State(state): axum::extract::State<State>) -> String {
    state.0.start().await.0
}

async fn stop(
    axum::extract::State(state): axum::extract::State<State>,
    axum::extract::Path(port): axum::extract::Path<String>,
) {
    state.0.stop(Port(port)).await;
}

#[derive(Debug, Clone)]
pub struct State(Arc<Processes>);
