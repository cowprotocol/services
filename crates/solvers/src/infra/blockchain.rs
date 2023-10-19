use std::time::Duration;

/// Creates a node RPC instance.
pub fn rpc(url: &reqwest::Url) -> ethrpc::Web3 {
    ethrpc::web3(
        Default::default(),
        reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .user_agent("cowprotocol-solver-engine/1.0.0"),
        url,
        "base",
    )
}
