use crate::domain::eth;
use ethereum_types::U256;
use itertools::Itertools as _;
use std::collections::BTreeMap;

/// The state of a Balancer-like weighted product pool.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Reserves,
    pub fee: eth::Rational,
}

impl Pool {
    /// Returns an iterator over the tokens pairs handled by this pool.
    pub fn token_pairs(&self) -> impl Iterator<Item = eth::TokenPair> + '_ {
        self.reserves
            .0
            .keys()
            .tuple_combinations()
            .map(|(a, b)| eth::TokenPair::new(*a, *b).expect("a != b"))
    }

    /// Retrieves a reserve by token.
    pub fn reserve(&self, token: eth::TokenAddress) -> Option<Reserve> {
        let (amount, weight, scale) = self.reserves.0.get(&token).copied()?;
        Some(Reserve {
            asset: eth::Asset { token, amount },
            weight,
            scale,
        })
    }
}

/// Reserve entry.
type ReserveEntry = (U256, eth::Rational, ScalingFactor);

/// A reprensentation of BalancerV2-like weighted pool reserves.
#[derive(Clone, Debug)]
pub struct Reserves(BTreeMap<eth::TokenAddress, ReserveEntry>);

impl Reserves {
    /// Returns an iterator over the token reserves.
    pub fn iter(&self) -> impl Iterator<Item = Reserve> + '_ {
        // Note that this uses a `BTreeMap` for internal storage. This is
        // because BalancerV2 weighted pools store their tokens in sorting order
        // - meaning that `token0` is the token address with the lowest sort
        // order. This ensures that this iterator returns the token reserves in
        // the correct order.
        self.0
            .iter()
            .map(|(&token, &(amount, weight, scale))| Reserve {
                asset: eth::Asset { token, amount },
                weight,
                scale,
            })
    }
}

/// A weighted pool token reserve.
pub struct Reserve {
    pub asset: eth::Asset,
    pub weight: eth::Rational,
    pub scale: ScalingFactor,
}

/// A scaling factor used for normalizing token amounts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScalingFactor(U256);

impl ScalingFactor {
    /// Creates a new scaling factor. Returns `None` if the value is not a power
    /// of 10.
    pub fn new(value: U256) -> Option<Self> {
        if !Self::is_power_of_10(value) {
            return None;
        }
        Some(Self(value))
    }

    /// Returns the underlying scaling factor value.
    pub fn get(&self) -> U256 {
        self.0
    }

    /// Returns the exponent of a scaling factor.
    pub fn exponent(&self) -> u8 {
        let mut factor = self.0;
        let mut exponent = 0_u8;
        while factor > U256::one() {
            factor /= 10;
            exponent += 1;
        }
        exponent
    }

    fn is_power_of_10(mut value: U256) -> bool {
        while value > U256::one() {
            let (quotient, remainder) = value.div_mod(10.into());
            if !remainder.is_zero() {
                return false;
            }
            value = quotient;
        }
        value == U256::one()
    }
}

impl Default for ScalingFactor {
    fn default() -> Self {
        Self(U256::one())
    }
}
