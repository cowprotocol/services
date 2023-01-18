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
//! [geth]: https://geth.ethereum.org/

use {
    std::{process::Stdio, sync::Arc},
    tokio::sync::Mutex,
};

const PORT: &str = "8547";
const GETH_PORT: &str = "8545";
const GETH_PROGRAM: &str = "geth";
const GETH_ARGS: [&str; 8] = [
    "--dev",
    "--http",
    "--http.addr",
    "0.0.0.0",
    "--http.api",
    "web3,eth,net,debug",
    "--http.port",
    GETH_PORT,
];

#[tokio::main]
async fn main() {
    run().await
}

// Avoid the IDE issues caused by #[tokio::main].
async fn run() {
    let child = Geth::start().await;

    let router: axum::Router<State> = axum::Router::new();
    let router = router.route("/", axum::routing::post(restart));
    let router = router.with_state(State(Arc::new(child)));

    axum::Server::bind(&format!("0.0.0.0:{PORT}").parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}

#[derive(Debug)]
struct Geth {
    child: Mutex<tokio::process::Child>,
}

impl Geth {
    async fn start() -> Self {
        Self {
            child: Mutex::new(Self::spawn().await),
        }
    }

    async fn restart(&self) {
        let mut child = self.child.lock().await;
        child.kill().await.unwrap();
        *child = Self::spawn().await;
    }

    async fn spawn() -> tokio::process::Child {
        let child = tokio::process::Command::new(dbg!(GETH_PROGRAM))
            .args(GETH_ARGS)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let web3 = web3::Web3::new(
            web3::transports::Http::new(&format!("http://localhost:{GETH_PORT}"))
                .expect("valid URL"),
        );
        loop {
            if web3.eth().accounts().await.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        child
    }
}

impl Drop for Geth {
    fn drop(&mut self) {
        tokio::runtime::Handle::current().block_on(async move {
            self.child.lock().await.kill().await.unwrap();
        });
    }
}

async fn restart(state: axum::extract::State<State>) {
    state.0 .0.restart().await;
}

#[derive(Debug, Clone)]
pub struct State(Arc<Geth>);
