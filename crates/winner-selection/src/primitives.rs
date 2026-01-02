//! Primitive types for winner selection.

pub use alloy::primitives::{Address, U256};

/// Native token constant.
pub const NATIVE_TOKEN: Address = Address::repeat_byte(0xee);

/// If the token is native, return the ERC20 wrapped version.
pub fn as_erc20(token: Address, wrapped: Address) -> Address {
    if token == NATIVE_TOKEN {
        wrapped
    } else {
        token
    }
}

/// Convert a token amount to ETH using this price.
///
/// Formula: `amount * price / 10^18`
pub fn price_in_eth(price: U256, amount: U256) -> U256 {
    amount.saturating_mul(price) / U256::from(1_000_000_000_000_000_000u64)
}

/// A directed token pair for tracking uniform clearing prices.
///
/// The direction matters: selling token A to buy token B is different from
/// selling token B to buy token A for the purpose of uniform directional
/// clearing prices.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DirectedTokenPair {
    pub sell: Address,
    pub buy: Address,
}

/// A unique identifier for an order.
///
/// This is a 56-byte array consisting of:
/// - 32 bytes: order digest (hash of order parameters)
/// - 20 bytes: owner address
/// - 4 bytes: valid until timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Order side (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Buy,
    Sell,
}

/// Protocol fee policy.
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote {
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee: U256,
    pub solver: Address,
}
