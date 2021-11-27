//! Balancer pool types.

use crate::{conversions::U256Ext, sources::balancer_v2::swap::fixed_point::Bfp};
use anyhow::{ensure, Result};
use ethcontract::{H160, H256, U256};
use num::BigRational;
use std::collections::BTreeMap;

/// A Balancer V2 pool.
///
/// This enum can represent any of the supported pool types.
pub enum Pool {
    /// A Balancer weighted pool.
    Weighted(WeightedPool),
    /// A Balancer stable (i.e. Curve-like) pool.
    Stable(StablePool),
}

#[derive(Clone, Debug)]
pub struct CommonPoolState {
    pub id: H256,
    pub address: H160,
    pub swap_fee: Bfp,
}

#[derive(Clone, Debug)]
pub struct WeightedPool {
    pub common: CommonPoolState,
    pub reserves: BTreeMap<H160, WeightedTokenState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedTokenState {
    pub common: TokenState,
    pub weight: Bfp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenState {
    pub balance: U256,
    pub scaling_exponent: u8,
}

#[derive(Clone, Debug)]
pub struct StablePool {
    pub common: CommonPoolState,
    pub reserves: BTreeMap<H160, TokenState>,
    pub amplification_parameter: AmplificationParameter,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AmplificationParameter {
    factor: U256,
    precision: U256,
}

impl AmplificationParameter {
    pub fn new(factor: U256, precision: U256) -> Result<Self> {
        ensure!(!precision.is_zero(), "Zero precision not allowed");
        Ok(Self { factor, precision })
    }

    /// This is the format used to pass into smart contracts.
    pub fn as_u256(&self) -> U256 {
        self.factor * self.precision
    }

    /// This is the format used to pass along to HTTP solver.
    pub fn as_big_rational(&self) -> BigRational {
        // We can assert that the precision is non-zero as we check when constructing
        // new `AmplificationParameter` instances that this invariant holds, and we don't
        // allow modifications of `self.precision` such that it could become 0.
        debug_assert!(!self.precision.is_zero());
        BigRational::new(self.factor.to_big_int(), self.precision.to_big_int())
    }
}
