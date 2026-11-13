//! Primitive types for winner selection, generic over the chain.

pub use {
    crate::evm::{Address, NATIVE_TOKEN, OrderUid, U256, as_erc20},
    protocol_fees::{FeePolicy, Quote, Side},
};

use crate::{chain::ChainTypes, evm::Evm};

/// A directed token pair for tracking uniform clearing prices.
///
/// The direction matters: selling token A to buy token B is different from
/// selling token B to buy token A for the purpose of uniform directional
/// clearing prices.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DirectedTokenPair<C: ChainTypes = Evm> {
    pub sell: C::TokenId,
    pub buy: C::TokenId,
}
