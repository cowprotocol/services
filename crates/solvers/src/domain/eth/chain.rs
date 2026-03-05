use {serde::Deserialize, std::str::FromStr};

/// A supported Ethereum Chain ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChainId {
    Mainnet = 1,
    Goerli = 5,
    Gnosis = 100,
    Base = 8453,
    ArbitrumOne = 42161,
    Bnb = 56,
    Avalanche = 43114,
    Optimism = 10,
    Polygon = 137,
    Linea = 59144,
    Plasma = 9745,
    Ink = 57073,
}

impl ChainId {
    pub fn new(value: u64) -> Result<Self, UnsupportedChain> {
        match value {
            1 => Ok(Self::Mainnet),
            5 => Ok(Self::Goerli),
            100 => Ok(Self::Gnosis),
            8453 => Ok(Self::Base),
            42161 => Ok(Self::ArbitrumOne),
            56 => Ok(Self::Bnb),
            43114 => Ok(Self::Avalanche),
            10 => Ok(Self::Optimism),
            137 => Ok(Self::Polygon),
            59144 => Ok(Self::Linea),
            9745 => Ok(Self::Plasma),
            57073 => Ok(Self::Ink),
            _ => Err(UnsupportedChain),
        }
    }

    /// Returns the network ID for the chain.
    pub fn network_id(self) -> &'static str {
        match self {
            ChainId::Mainnet => "1",
            ChainId::Goerli => "5",
            ChainId::Gnosis => "100",
            ChainId::Base => "8453",
            ChainId::ArbitrumOne => "42161",
            ChainId::Bnb => "56",
            ChainId::Avalanche => "43114",
            ChainId::Optimism => "10",
            ChainId::Polygon => "137",
            ChainId::Linea => "59144",
            ChainId::Plasma => "9745",
            ChainId::Ink => "57073",
        }
    }
}

impl<'de> Deserialize<'de> for ChainId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ChainIdVisitor;

        impl<'de> serde::de::Visitor<'de> for ChainIdVisitor {
            type Value = ChainId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a chain ID as a string or number")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let chain_id = u64::from_str(v).map_err(E::custom)?;
                ChainId::new(chain_id).map_err(E::custom)
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ChainId::new(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(ChainIdVisitor)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unsupported chain")]
pub struct UnsupportedChain;
