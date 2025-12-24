//! Primitive types for winner selection.

// Re-export alloy primitives
pub use alloy::primitives::{Address as EthAddress, U256 as EthU256};
use {
    alloy::primitives::{Address, U256},
    derive_more::{Display, From, Into},
};

/// Native token constant (ETH on mainnet, XDAI on Gnosis)
pub const NATIVE_TOKEN: TokenAddress = TokenAddress(Address::repeat_byte(0xee));

/// An ERC20 token address.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Into,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct TokenAddress(pub Address);

impl TokenAddress {
    /// If the token is ETH/XDAI, return WETH/WXDAI, converting it to ERC20.
    pub fn as_erc20(self, wrapped: WrappedNativeToken) -> Self {
        if self == NATIVE_TOKEN {
            wrapped.into()
        } else {
            self
        }
    }
}

/// ERC20 representation of the chain's native token (WETH on mainnet, WXDAI on
/// Gnosis).
#[derive(Debug, Clone, Copy, From, Into, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct WrappedNativeToken(pub TokenAddress);

impl From<Address> for WrappedNativeToken {
    fn from(value: Address) -> Self {
        WrappedNativeToken(value.into())
    }
}

/// An ERC20 token amount.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Into,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct TokenAmount(pub U256);

/// An amount denominated in the native token (ETH/XDAI).
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Into,
    Display,
    derive_more::Add,
    derive_more::Sub,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct Ether(pub U256);

impl Ether {
    pub const ZERO: Self = Self(U256::ZERO);

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

/// A price for converting token amounts to native token (ETH/XDAI).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, From, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Price(pub Ether);

impl Price {
    /// Convert a token amount to ETH using this price.
    ///
    /// Formula: `amount * price / 10^18`
    pub fn in_eth(&self, amount: TokenAmount) -> Ether {
        // Compute (amount * price) / 10^18
        // Use saturating operations to avoid overflow
        let product = amount.0.saturating_mul(self.0.0);
        let eth_amount = product / U256::from(1_000_000_000_000_000_000u64); // 10^18
        Ether(eth_amount)
    }
}

/// A solution score in native token.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Display,
    derive_more::Add,
    derive_more::Sub,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct Score(pub Ether);

impl Score {
    /// Create a new score, returning an error if it's zero.
    pub fn new(ether: Ether) -> Result<Self, ZeroScore> {
        if ether.0.is_zero() {
            Err(ZeroScore)
        } else {
            Ok(Self(ether))
        }
    }

    /// Get the inner Ether value.
    pub fn get(&self) -> &Ether {
        &self.0
    }

    pub fn saturating_add_assign(&mut self, other: Self) {
        self.0 = self.0.saturating_add(other.0);
    }
}

impl num::Saturating for Score {
    fn saturating_add(self, v: Self) -> Self {
        Self(self.0.saturating_add(v.0))
    }

    fn saturating_sub(self, v: Self) -> Self {
        Self(self.0.saturating_sub(v.0))
    }
}

impl num::CheckedSub for Score {
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        self.0.checked_sub(v.0).map(Score)
    }
}

/// Error returned when a score is zero.
#[derive(Debug, thiserror::Error)]
#[error("the solver proposed a 0-score solution")]
pub struct ZeroScore;

/// A directed token pair for tracking uniform clearing prices.
///
/// The direction matters: selling token A to buy token B is different from
/// selling token B to buy token A for the purpose of uniform directional
/// clearing prices.
#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DirectedTokenPair {
    pub sell: TokenAddress,
    pub buy: TokenAddress,
}

/// A unique identifier for an order.
///
/// This is a 56-byte array consisting of:
/// - 32 bytes: order digest (hash of order parameters)
/// - 20 bytes: owner address
/// - 4 bytes: valid until timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderUid(pub [u8; 56]);

impl OrderUid {
    /// Extract the owner address from the order UID.
    pub fn owner(&self) -> Address {
        // Bytes 32-51 contain the owner address (20 bytes)
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&self.0[32..52]);
        Address::from(bytes)
    }
}

impl serde::Serialize for OrderUid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as hex string with 0x prefix
        let hex_string = format!("0x{}", hex::encode(self.0));
        serializer.serialize_str(&hex_string)
    }
}

impl<'de> serde::Deserialize<'de> for OrderUid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let decoded = hex::decode(s).map_err(serde::de::Error::custom)?;
        if decoded.len() != 56 {
            return Err(serde::de::Error::custom(format!(
                "expected 56 bytes, got {}",
                decoded.len()
            )));
        }
        let mut bytes = [0u8; 56];
        bytes.copy_from_slice(&decoded);
        Ok(OrderUid(bytes))
    }
}

/// Order side (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

/// Protocol fee policy.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FeePolicy {
    /// Fee as a percentage of surplus over limit price.
    Surplus { factor: f64, max_volume_factor: f64 },
    /// Fee as a percentage of price improvement over quote.
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote,
    },
    /// Fee as a percentage of order volume.
    Volume { factor: f64 },
}

/// Quote data for price improvement fee calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Quote {
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee: U256,
    pub solver: Address,
}
