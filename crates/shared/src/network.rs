use std::time::Duration;

/// Maps (NetworkId, ChainId) to the network name.
/// If the output is from a known network, it represents the canonical name of
/// the network on CoW Protocol.
pub fn network_name(network_id: &str, chain_id: u64) -> &'static str {
    // You can find a list of available networks by network and chain id here:
    // https://chainid.network/chains.json
    match (network_id, chain_id) {
        ("1", 1) => "Ethereum / Mainnet",
        ("5", 5) => "Ethereum / Goerli",
        ("100", 100) => "xDAI",
        ("11155111", 11155111) => "Ethereum / Sepolia",
        _ => panic!("Unknown network (network_id={network_id}, chain_id={chain_id})"),
    }
}

/// The expected time between blocks on the network.
pub fn block_interval(network_id: &str, chain_id: u64) -> Option<Duration> {
    Some(Duration::from_secs(match (network_id, chain_id) {
        ("1", 1) => 12,
        ("5", 5) => 12,
        ("100", 100) => 5,
        ("11155111", 11155111) => 12,
        _ => return None,
    }))
}
