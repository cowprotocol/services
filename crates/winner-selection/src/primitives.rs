//! Primitive types for winner selection, generic over the chain.

pub use crate::evm::{Address, NATIVE_TOKEN, OrderUid, U256, as_erc20};
use crate::{chain::Scoring, evm::Evm};

/// A directed token pair for tracking uniform clearing prices.
///
/// The direction matters: selling token A to buy token B is different from
/// selling token B to buy token A for the purpose of uniform directional
/// clearing prices.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DirectedTokenPair<C: Scoring = Evm> {
    pub sell: C::TokenId,
    pub buy: C::TokenId,
}

/// Order side (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Buy,
    Sell,
}

/// Protocol fee policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeePolicy<C: Scoring = Evm> {
    /// Fee as a percentage of surplus over limit price.
    Surplus { factor: f64, max_volume_factor: f64 },
    /// Fee as a percentage of price improvement over quote.
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote<C>,
    },
    /// Fee as a percentage of order volume.
    Volume { factor: f64 },
}

/// Quote data for price improvement fee calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote<C: Scoring = Evm> {
    pub sell_amount: C::Amount,
    pub buy_amount: C::Amount,
    pub fee: C::Amount,
    pub solver: C::AccountId,
}
