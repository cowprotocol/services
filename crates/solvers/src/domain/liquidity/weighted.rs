use crate::domain::{eth, liquidity};
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
type ReserveEntry = (U256, eth::Rational, liquidity::balancer::ScalingFactor);

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
    pub scale: liquidity::balancer::ScalingFactor,
}
