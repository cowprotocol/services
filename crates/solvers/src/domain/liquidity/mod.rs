//! Modelling on-chain liquidity.

pub mod concentrated;
pub mod constant_product;
pub mod limit_order;
pub mod stable;
pub mod weighted_product;

use {crate::domain::eth, std::cmp::Ordering};

/// A source of liquidity which can be used by the solver.
#[derive(Clone, Debug)]
pub struct Liquidity {
    pub id: Id,
    pub address: eth::Address,
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
    Stable(stable::Pool),
    Concentrated(concentrated::Pool),
    LimitOrder(limit_order::LimitOrder),
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

/// A scaling factor used for normalizing token amounts.
#[derive(Clone, Copy, Debug)]
pub struct ScalingFactor(eth::Rational);

impl ScalingFactor {
    /// Creates a new scaling factor. Returns `None` if the specified value is
    /// 0 (as a 0 scaling factor is not allowed).
    pub fn new(value: eth::Rational) -> Option<Self> {
        if value.numer().is_zero() || value.denom().is_zero() {
            return None;
        }
        Some(Self(value))
    }

    /// Returns the underlying scaling factor value.
    pub fn get(&self) -> eth::Rational {
        self.0
    }
}

impl Default for ScalingFactor {
    fn default() -> Self {
        Self(eth::Rational::new_raw(eth::U256::ONE, eth::U256::ONE))
    }
}
