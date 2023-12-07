use std::time::Duration;

/// Maps (NetworkId, ChainId) to the network name.
/// If the output is from a known network, it represents the canonical name of
/// the network on CoW Protocol.
pub fn network_name(network_id: &str, chain_id: u64) -> String {
    // You can find a list of available networks by network and chain id here:
    // https://chainid.network/chains.json
    (match (network_id, chain_id) {
        ("1", 1) => "Ethereum / Mainnet",
        ("5", 5) => "Ethereum / Goerli",
        ("100", 100) => "xDAI",
        ("11155111", 11155111) => "Ethereum / Sepolia",
        _ => return format!("<unknown network network_id={network_id} chain_id={chain_id}>"),
    })
    .to_string()
}

/// The expected time between blocks on the network.
pub fn block_interval(network_id: &str, chain_id: u64) -> Option<Duration> {
    Some(Duration::from_secs(match (network_id, chain_id) {
        ("1", 1) => 12,
        ("5", 5) => 12,
        ("100", 100) => 5,
        _ => return None,
    }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn unknown_network_specifies_ids_in_name() {
        assert_eq!(
            // These two numbers were generated at random to make a collision unlikely in the
            // future.
            network_name("73069754018", 25327196146),
            "<unknown network network_id=73069754018 chain_id=25327196146>"
        )
    }
}
