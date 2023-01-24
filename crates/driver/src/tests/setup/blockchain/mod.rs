use ethcontract::{dyns::DynWeb3, transport::DynTransport, Web3};

pub mod uniswap;

use futures::Future;
pub use uniswap::Uniswap;

/// The URL to which a post request can be made to start and stop geth
/// instances. See the `dev-geth` crate.
const DEV_GETH_PORT: &str = "8547";

pub fn web3(url: &str) -> DynWeb3 {
    Web3::new(DynTransport::new(
        web3::transports::Http::new(url).expect("valid URL"),
    ))
}

/// Get the first account owned by the web3 node.
pub async fn primary_address(web3: &DynWeb3) -> ethcontract::H160 {
    web3.eth().accounts().await.unwrap()[0]
}

/// An instance of geth managed by `dev-geth`. When this type is dropped, the
/// geth instance gets shut down.
#[derive(Debug)]
pub struct Geth {
    port: String,
}

impl Geth {
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}

impl Drop for Geth {
    fn drop(&mut self) {
        let port = std::mem::take(&mut self.port);
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            client
                .delete(&format!("http://localhost:{DEV_GETH_PORT}/{}", port))
                .send()
                .await
                .unwrap();
        });
    }
}

/// Setup a new geth instance.
pub async fn geth() -> Geth {
    let http = reqwest::Client::new();
    let res = http
        .post(format!("http://localhost:{DEV_GETH_PORT}"))
        .send()
        .await
        .unwrap();
    let port = res.text().await.unwrap();
    Geth { port }
}

/// Execute an asynchronous operation, then wait for the next block to be mined
/// before proceeding.
///
/// [Dev mode geth](https://geth.ethereum.org/docs/developers/dapp-developer/dev-mode)
/// mines blocks as soon as there's a pending transaction, but publishing a
/// transaction does not wait for the block to be mined before returning. This
/// introduces a subtle race condition, so it's necessary to
/// wait for transactions to be confirmed before proceeding with the test. When
/// switching from geth back to hardhat, this function can be removed.
pub async fn wait_for<T>(web3: &DynWeb3, fut: impl Future<Output = T>) -> T {
    let block = web3.eth().block_number().await.unwrap();
    let result = fut.await;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let next_block = web3.eth().block_number().await.unwrap();
            if next_block > block {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("timeout while waiting for next block to be mined");
    result
}
