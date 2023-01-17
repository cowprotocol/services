use ethcontract::{dyns::DynWeb3, transport::DynTransport, Web3};

pub mod uniswap;

pub use uniswap::Uniswap;

pub const WEB3_URL: &str = "http://localhost:8546";
/// The URL to which a post request can be made to reset the geth development
/// environment. See the `dev-geth` crate.
const DEV_GETH_URL: &str = "http://localhost:8547";

pub fn web3() -> DynWeb3 {
    Web3::new(DynTransport::new(
        web3::transports::Http::new(WEB3_URL).expect("valid URL"),
    ))
}

/// Get the first account owned by the web3 node.
pub async fn primary_address(web3: &DynWeb3) -> ethcontract::H160 {
    web3.eth().accounts().await.unwrap()[0]
}

/// Reset the blockchain state.
async fn reset() {
    let http = reqwest::Client::new();
    http.post(DEV_GETH_URL).send().await.unwrap();
}
