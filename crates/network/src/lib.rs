use {
    ethcontract::{
        jsonrpc::serde::{de, Deserialize, Deserializer},
        U256,
    },
    thiserror::Error,
};

/// Represents each available network
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u64)]
pub enum Network {
    Mainnet = 1,
    Goerli = 5,
    Gnosis = 100,
    Sepolia = 11155111,
    ArbitrumOne = 42161,
    Base = 8453,
}

impl Network {
    /// Returns the network's chain ID
    pub fn chain_id(&self) -> u64 {
        *self as u64
    }

    /// Returns the canonical name of the network on CoW Protocol.
    pub fn name(&self) -> &'static str {
        // You can find a list of available networks by network and chain id here:
        // https://chainid.network/chains.json
        match &self {
            Self::Mainnet => "Ethereum / Mainnet",
            Self::Goerli => "Ethereum / Goerli",
            Self::Gnosis => "xDAI",
            Self::Sepolia => "Ethereum / Sepolia",
            Self::ArbitrumOne => "Arbitrum One",
            Self::Base => "Base",
        }
    }

    pub fn default_amount_to_estimate_native_prices_with(&self) -> Option<U256> {
        match &self {
            Self::Mainnet | Self::Goerli | Self::Sepolia | Self::ArbitrumOne | Self::Base => {
                Some(10u128.pow(18).into())
            }
            Self::Gnosis => Some(10u128.pow(21).into()),
        }
    }

    /// Returns the block time in milliseconds
    pub fn block_time_in_ms(&self) -> u64 {
        match self {
            Self::Mainnet => 12_000,
            Self::Goerli => 12_000,
            Self::Gnosis => 5_000,
            Self::Sepolia => 12_000,
            Self::ArbitrumOne => 250,
            Self::Base => 2_000,
        }
    }

    /// Returns the number of blocks that fits into the given time (in
    /// milliseconds)
    pub fn blocks_in(&self, time_in_ms: u64) -> f64 {
        time_in_ms as f64 / self.block_time_in_ms() as f64
    }
}

impl TryFrom<u64> for Network {
    type Error = Error;

    /// Initializes `Network` from a chain ID, returns error if the chain id is
    /// not supported
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let network = match value {
            x if x == Self::Mainnet as u64 => Self::Mainnet,
            x if x == Self::Goerli as u64 => Self::Goerli,
            x if x == Self::Gnosis as u64 => Self::Gnosis,
            x if x == Self::Sepolia as u64 => Self::Sepolia,
            x if x == Self::ArbitrumOne as u64 => Self::ArbitrumOne,
            x if x == Self::Base as u64 => Self::Base,
            _ => Err(Error::ChainIdNotSupported)?,
        };
        Ok(network)
    }
}

impl TryFrom<U256> for Network {
    type Error = Error;

    /// Initializes `Network` from a chain ID, returns error if the chain id is
    /// not supported
    fn try_from(value: U256) -> Result<Self, Self::Error> {
        // Check to avoid panics for large `U256` values, as there is no checked
        // conversion API available, and we don't support chains with IDs greater
        // than `u64::MAX` anyway.
        if value > U256::from(u64::MAX) {
            return Err(Error::ChainIdNotSupported);
        }
        value.as_u64().try_into()
    }
}

impl<'de> Deserialize<'de> for Network {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NetworkVisitor;

        impl<'de> de::Visitor<'de> for NetworkVisitor {
            type Value = Network;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a u64 or a string")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Network::try_from(value).map_err(de::Error::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Network::try_from(value.parse::<u64>().map_err(de::Error::custom)?)
                    .map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_any(NetworkVisitor)
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("chain id not supported")]
    ChainIdNotSupported,
}

#[cfg(test)]
mod test {
    use {super::*, ethcontract::jsonrpc::serde_json};

    #[test]
    fn test_number_of_blocks_in() {
        const TARGET_AGE: u64 = 6 * 60 * 60 * 1000; // 6h in ms

        assert_eq!(Network::Mainnet.blocks_in(TARGET_AGE).round(), 1662.0);
        assert_eq!(Network::Sepolia.blocks_in(TARGET_AGE).round(), 1662.0);
        assert_eq!(Network::Goerli.blocks_in(TARGET_AGE).round(), 1662.0);
        assert_eq!(Network::Gnosis.blocks_in(TARGET_AGE).round(), 4320.0);
        assert_eq!(Network::Base.blocks_in(TARGET_AGE).round(), 10800.0);
        assert_eq!(Network::ArbitrumOne.blocks_in(TARGET_AGE).round(), 86400.0);
    }

    #[test]
    fn test_deserialize_from_u64() {
        // Test valid u64 deserialization
        let json_data = "1"; // Should deserialize to Network::Mainnet
        let network: Network = serde_json::from_str(json_data).unwrap();
        assert_eq!(network, Network::Mainnet);

        let json_data = "5"; // Should deserialize to Network::Goerli
        let network: Network = serde_json::from_str(json_data).unwrap();
        assert_eq!(network, Network::Goerli);

        let json_data = "100"; // Should deserialize to Network::Gnosis
        let network: Network = serde_json::from_str(json_data).unwrap();
        assert_eq!(network, Network::Gnosis);

        // Test invalid u64 deserialization (should return an error)
        let json_data = "9999999"; // Not a valid Network variant
        let result: Result<Network, _> = serde_json::from_str(json_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_from_str() {
        // Test valid string deserialization
        let json_data = "\"1\""; // Should parse to u64 1 and then to Network::Mainnet
        let network: Network = serde_json::from_str(json_data).unwrap();
        assert_eq!(network, Network::Mainnet);

        let json_data = "\"5\""; // Should parse to u64 5 and then to Network::Goerli
        let network: Network = serde_json::from_str(json_data).unwrap();
        assert_eq!(network, Network::Goerli);

        let json_data = "\"100\""; // Should parse to u64 100 and then to Network::Gnosis
        let network: Network = serde_json::from_str(json_data).unwrap();
        assert_eq!(network, Network::Gnosis);

        // Test invalid string deserialization (should return an error)
        let json_data = "\"invalid\""; // Cannot be parsed as u64
        let result: Result<Network, _> = serde_json::from_str(json_data);
        assert!(result.is_err());
    }
}
