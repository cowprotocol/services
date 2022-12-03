// TODO Constructing this type should probably do some validation, or maybe this
// should be an enum with a Display implementation
/// Name of an Ethereum network, e.g. mainnet or testnet.
pub struct NetworkName(pub String);

/// Chain ID as defined by EIP-155.
///
/// https://eips.ethereum.org/EIPS/eip-155
pub struct ChainId(pub u64);

impl From<String> for NetworkName {
    fn from(inner: String) -> Self {
        Self(inner)
    }
}

impl From<u64> for ChainId {
    fn from(inner: u64) -> Self {
        Self(inner)
    }
}
