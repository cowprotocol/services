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
    std::{
        process::Stdio,
        sync::{Arc, Mutex},
    },
};

const PORT: &str = "8547";
const GETH_PROGRAM: &str = "geth";
const GETH_ARGS: &[&str] = &[
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

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Port(u16);

impl Default for Port {
    fn default() -> Self {
        Self::LAST
    }
}

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Port {
    /// The port range from [`FIRST`] to [`LAST`] is expected to be controlled
    /// by `dev-geth` and not be touched by any other process.
    const FIRST: Self = Port(18545);
    const LAST: Self = Port(19545);

    fn next(self) -> Self {
        if self >= Self::LAST {
            Self::FIRST
        } else {
            Self(self.0 + 1)
        }
    }
}

#[derive(Debug, Default)]
struct Processes {
    last_port: Mutex<Port>,
    children: DashMap<Port, tokio::process::Child>,
}

impl Processes {
    async fn start(&self) -> Port {
        let port = self.next_port();
        let datadir = format!("/{}", port);
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
        let child = tokio::process::Command::new(GETH_PROGRAM)
            .args(GETH_ARGS.iter().map(ToOwned::to_owned).chain([
                "--http.port",
                &port.to_string(),
                "--datadir",
                &datadir,
                "--authrpc.port",
                &self.next_port().to_string(),
            ]))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        Self::wait_for_geth(&port).await;
        self.children.insert(port, child);
        port
    }

    async fn stop(&self, port: Port) {
        let (_, mut process) = self
            .children
            .remove(&port)
            .expect("no process running at given port");
        tokio::fs::remove_dir_all(format!("/{}", port.0))
            .await
            .expect("failed to delete the datadir");
        process.kill().await.unwrap();
    }

    fn next_port(&self) -> Port {
        let mut last_port = self.last_port.lock().unwrap();
        let port = last_port.next();
        *last_port = port;
        port
    }

    async fn wait_for_geth(port: &Port) {
        let web3 = web3::Web3::new(
            web3::transports::Http::new(&format!("http://localhost:{}", port.0))
                .expect("valid URL"),
        );
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            for i in 1.. {
                if let Ok(Ok(..)) = tokio::time::timeout(
                    std::time::Duration::from_millis(50 * i),
                    web3.eth().accounts(),
                )
                .await
                {
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
    state.0.start().await.0.to_string()
}

async fn stop(
    axum::extract::State(state): axum::extract::State<State>,
    axum::extract::Path(port): axum::extract::Path<String>,
) {
    state.0.stop(Port(port.parse().unwrap())).await;
}

#[derive(Debug, Clone)]
pub struct State(Arc<Processes>);
