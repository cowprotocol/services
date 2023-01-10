use ethcontract::{dyns::DynWeb3, transport::DynTransport, Web3};

pub mod uniswap;

pub use uniswap::Uniswap;

pub const WEB3_URL: &str = "http://localhost:8546";

pub fn web3() -> Web3<DynTransport> {
    Web3::new(DynTransport::new(
        web3::transports::Http::new(WEB3_URL).expect("valid URL"),
    ))
}

/// Get the first account owned by the web3 node.
pub async fn primary_account(web3: &DynWeb3) -> ethcontract::H160 {
    web3.eth().accounts().await.unwrap()[0]
}
