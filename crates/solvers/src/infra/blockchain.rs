/// Creates a node RPC instance.
pub fn rpc(url: &reqwest::Url) -> ethrpc::Web3 {
    ethrpc::web3(Default::default(), url, Some("base"))
}
