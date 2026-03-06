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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_chains_number() {
        assert_eq!(
            serde_json::from_value::<ChainId>(1.into()).unwrap(),
            ChainId::Mainnet
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(5.into()).unwrap(),
            ChainId::Goerli
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(100.into()).unwrap(),
            ChainId::Gnosis
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(8453.into()).unwrap(),
            ChainId::Base
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(42161.into()).unwrap(),
            ChainId::ArbitrumOne
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(56.into()).unwrap(),
            ChainId::Bnb
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(43114.into()).unwrap(),
            ChainId::Avalanche
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(10.into()).unwrap(),
            ChainId::Optimism
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(137.into()).unwrap(),
            ChainId::Polygon
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(59144.into()).unwrap(),
            ChainId::Linea
        );
        assert_eq!(
            serde_json::from_value::<ChainId>(9745.into()).unwrap(),
            ChainId::Plasma
        );
    }

    #[test]
    fn supported_chains_str() {
        assert_eq!(
            serde_json::from_str::<ChainId>("1").unwrap(),
            ChainId::Mainnet
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("5").unwrap(),
            ChainId::Goerli
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("100").unwrap(),
            ChainId::Gnosis
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("8453").unwrap(),
            ChainId::Base
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("42161").unwrap(),
            ChainId::ArbitrumOne
        );
        assert_eq!(serde_json::from_str::<ChainId>("56").unwrap(), ChainId::Bnb);
        assert_eq!(
            serde_json::from_str::<ChainId>("43114").unwrap(),
            ChainId::Avalanche
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("10").unwrap(),
            ChainId::Optimism
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("137").unwrap(),
            ChainId::Polygon
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("59144").unwrap(),
            ChainId::Linea
        );
        assert_eq!(
            serde_json::from_str::<ChainId>("9745").unwrap(),
            ChainId::Plasma
        );
    }

    #[test]
    fn unsupported_chains() {
        serde_json::from_str::<ChainId>("0").unwrap_err();
    }
}
