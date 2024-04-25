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
        42161 => "Arbitrum / Mainnet",
        _ => panic!("Unknown network (chain_id={chain_id})"),
    }
}

/// The expected time between blocks on the network.
pub fn block_interval(chain_id: u64) -> Option<Duration> {
    match chain_id {
        1 => Duration::from_secs(12),
        5 => Duration::from_secs(12),
        100 => Duration::from_secs(5),
        11155111 => Duration::from_secs(12),
        42161 => Duration::from_millis(250),
        _ => return None,
    }
    .into()
}
