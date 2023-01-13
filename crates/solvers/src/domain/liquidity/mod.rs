//! Modelling on-chain liquidity.

pub mod constant_product;
pub mod weighted_product;

use crate::domain::eth;
use ethereum_types::H160;
use std::cmp::Ordering;

/// A source of liquidity which can be used by the solver.
#[derive(Clone, Debug)]
pub struct Liquidity {
    pub id: Id,
    pub address: H160,
    /// Estimation of gas needed to use this liquidity on-chain.
    pub gas: eth::Gas,
    pub state: State,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub String);

/// The liquidity state, specific to the type of liquidity.
#[derive(Clone, Debug)]
pub enum State {
    ConstantProduct(constant_product::Pool),
    WeightedProduct(weighted_product::Pool),
}

/// An ordered token pair.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TokenPair(eth::TokenAddress, eth::TokenAddress);

impl TokenPair {
    /// Returns a token pair for the given tokens, or `None` if `a` and `b` are
    /// equal.
    pub fn new(a: eth::TokenAddress, b: eth::TokenAddress) -> Option<Self> {
        match a.cmp(&b) {
            Ordering::Less => Some(Self(a, b)),
            Ordering::Equal => None,
            Ordering::Greater => Some(Self(b, a)),
        }
    }

    /// Returns the wrapped token pair as a tuple.
    pub fn get(&self) -> (eth::TokenAddress, eth::TokenAddress) {
        (self.0, self.1)
    }
}
