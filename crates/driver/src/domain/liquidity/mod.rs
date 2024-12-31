// TODO Remove dead_code
#![allow(dead_code)]

use {
    crate::domain::eth,
    derive_more::{From, Into},
    std::cmp::Ordering,
};

pub mod balancer;
pub mod swapr;
pub mod uniswap;
pub mod zeroex;

/// A source of liquidity which can be used by the solver.
#[derive(Debug, Clone)]
pub struct Liquidity {
    pub id: Id,
    /// Estimation of gas needed to use this liquidity on-chain.
    pub gas: eth::Gas,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, From, Into)]
pub struct Id(pub usize);

impl PartialEq<usize> for Id {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}

/// A limit input amount.
#[derive(Clone, Copy, Debug)]
pub struct MaxInput(pub eth::Asset);

/// An exact output amount.
#[derive(Clone, Copy, Debug)]
pub struct ExactOutput(pub eth::Asset);

/// Data tied to a particular liquidity instance, specific to the kind of
/// liquidity.
///
/// This contains relevant data for encoding interactions for the given
/// liquidity, as well as state required by the solver engine.
#[derive(Debug, Clone)]
pub enum Kind {
    UniswapV2(uniswap::v2::Pool),
    UniswapV3(uniswap::v3::Pool),
    BalancerV2Stable(balancer::v2::stable::Pool),
    BalancerV2Weighted(balancer::v2::weighted::Pool),
    Swapr(swapr::Pool),
    ZeroEx(zeroex::LimitOrder),
}

impl From<&Kind> for &'static str {
    fn from(val: &Kind) -> &'static str {
        match *val {
            Kind::UniswapV2(_) => "UniswapV2",
            Kind::UniswapV3(_) => "UniswapV3",
            Kind::BalancerV2Stable(_) => "BalancerV2Stable",
            Kind::BalancerV2Weighted(_) => "BalancerV2Weighted",
            Kind::Swapr(_) => "Swapr",
            Kind::ZeroEx(_) => "ZeroExLimitOrder",
        }
    }
}

/// An ordered token pair.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TokenPair(eth::TokenAddress, eth::TokenAddress);

impl TokenPair {
    /// Returns a token pair for the given tokens, or `Err` if `a` and `b` are
    /// equal.
    pub fn try_new(a: eth::TokenAddress, b: eth::TokenAddress) -> Result<Self, InvalidTokenPair> {
        match a.cmp(&b) {
            Ordering::Less => Ok(Self(a, b)),
            Ordering::Equal => Err(InvalidTokenPair),
            Ordering::Greater => Ok(Self(b, a)),
        }
    }

    /// Returns the wrapped token pair as a tuple.
    pub fn get(&self) -> (eth::TokenAddress, eth::TokenAddress) {
        (self.0, self.1)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("token pair must have distict token addresses")]
pub struct InvalidTokenPair;

#[derive(Debug, thiserror::Error)]
#[error("swap parameters do not match pool")]
pub struct InvalidSwap;
