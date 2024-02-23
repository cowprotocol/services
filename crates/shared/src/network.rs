use std::time::Duration;

/// Maps ChainId to the network name.
/// If the output is from a known network, it represents the canonical name of
/// the network on CoW Protocol.
pub fn network_name(chain_id: u64) -> &'static str {
    // You can find a list of available networks by network and chain id here:
    // https://chainid.network/chains.json
    match chain_id {
        1 => "Ethereum / Mainnet",
        5 => "Ethereum / Goerli",
        100 => "xDAI",
        11155111 => "Ethereum / Sepolia",
        _ => panic!("Unknown network (chain_id={chain_id})"),
    }
}

/// The expected time between blocks on the network.
pub fn block_interval(chain_id: u64) -> Option<Duration> {
    Some(Duration::from_secs(match chain_id {
        1 => 12,
        5 => 12,
        100 => 5,
        11155111 => 12,
        _ => return None,
    }))
}
