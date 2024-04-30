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
        42161 => "Arbitrum One",
        _ => panic!("Unknown network (chain_id={chain_id})"),
    }
}
