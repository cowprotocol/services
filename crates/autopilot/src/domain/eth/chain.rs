use primitive_types::U256;

/// A supported Ethereum Chain ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChainId {
    Mainnet = 1,
    Gnosis = 100,
    Sepolia = 11155111,
    ArbitrumOne = 42161,
}

impl ChainId {
    pub fn new(value: U256) -> Result<Self, UnsupportedChain> {
        // Check to avoid panics for large `U256` values, as there is no checked
        // conversion API available and we don't support chains with IDs greater
        // than `u64::MAX` anyway.
        if value > U256::from(u64::MAX) {
            return Err(UnsupportedChain);
        }

        match value.as_u64() {
            1 => Ok(Self::Mainnet),
            100 => Ok(Self::Gnosis),
            11155111 => Ok(Self::Sepolia),
            42161 => Ok(Self::ArbitrumOne),
            _ => Err(UnsupportedChain),
        }
    }

    /// Returns the network ID for the chain.
    pub fn network_id(self) -> &'static str {
        match self {
            ChainId::Mainnet => "1",
            ChainId::Gnosis => "100",
            ChainId::Sepolia => "11155111",
            ChainId::ArbitrumOne => "42161",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unsupported chain")]
pub struct UnsupportedChain;
