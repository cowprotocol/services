use {
    super::{Fee, Id, ScalingFactor},
    crate::{
        boundary,
        domain::{eth, liquidity},
    },
    itertools::Itertools,
};

/// Liquidity data tied to a Balancer V2 pool based on "Weighted Math" [^1].
///
/// Balancer V2 supports two kinds of pools that fall in this category:
/// - Weighted Pools [^2]
/// - Liquidity Bootstrapping Pools [^3]
///
/// Both of these pools have an identical representation, and are therefore
/// modelled by the same type.
///
/// [^1]: <https://docs.balancer.fi/concepts/math/weighted-math>
/// [^2]: <https://docs.balancer.fi/products/balancer-pools/weighted-pools>
/// [^3]: <https://docs.balancer.fi/products/balancer-pools/liquidity-bootstrapping-pools-lbps>
#[derive(Clone, Debug)]
pub struct Pool {
    pub vault: eth::ContractAddress,
    pub id: Id,
    pub reserves: Reserves,
    pub fee: Fee,
    pub version: Version,
}

impl Pool {
    /// Encodes a pool swap as an interaction. Returns `Err` if the swap
    /// parameters are invalid for the pool, specifically if the input and
    /// output tokens do not belong to the pool.
    pub fn swap(
        &self,
        input: &liquidity::MaxInput,
        output: &liquidity::ExactOutput,
        receiver: &eth::Address,
    ) -> Result<eth::Interaction, liquidity::InvalidSwap> {
        if !self.reserves.has_tokens(&input.0.token, &output.0.token) {
            return Err(liquidity::InvalidSwap);
        }

        Ok(boundary::liquidity::balancer::v2::weighted::to_interaction(
            self, input, output, receiver,
        ))
    }
}

/// Balancer weighted pool reserves.
///
/// This is an ordered collection of tokens with their balance and weights.
#[derive(Clone, Debug)]
pub struct Reserves(Vec<Reserve>);

impl Reserves {
    /// Creates new Balancer V2 token reserves, returns `Err` if the specified
    /// token reserves are invalid.
    pub fn new(reserves: Vec<Reserve>) -> Result<Self, InvalidReserves> {
        if !reserves.iter().map(|r| r.asset.token).all_unique() {
            return Err(InvalidReserves::DuplicateToken);
        }

        let total_weight = reserves.iter().fold(eth::U256::default(), |acc, r| {
            acc.saturating_add(r.weight.0)
        });
        if total_weight != Weight::base() {
            return Err(InvalidReserves::AbnormalWeights);
        }

        Ok(Self(reserves))
    }

    /// Returns `true` if the reserves correspond to the specified tokens.
    fn has_tokens(&self, a: &eth::TokenAddress, b: &eth::TokenAddress) -> bool {
        self.tokens().contains(a) && self.tokens().contains(b)
    }

    /// Returns an iterator over the reserve tokens.
    pub fn tokens(&self) -> impl Iterator<Item = eth::TokenAddress> + '_ {
        self.iter().map(|r| r.asset.token)
    }

    /// Returns an iterator over the reserve assets.
    pub fn iter(&self) -> impl Iterator<Item = Reserve> + '_ {
        self.0.iter().copied()
    }
}

impl IntoIterator for Reserves {
    type IntoIter = <Vec<Reserve> as IntoIterator>::IntoIter;
    type Item = Reserve;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidReserves {
    #[error("invalid Balancer V2 token reserves; duplicate token address")]
    DuplicateToken,

    #[error("invalid Balancer V2 token reserves; token weights do not sum to 1.0")]
    AbnormalWeights,
}

/// Balancer weighted pool reserve for a single token.
#[derive(Clone, Copy, Debug)]
pub struct Reserve {
    pub asset: eth::Asset,
    pub scale: ScalingFactor,
    pub weight: Weight,
}

/// A Balancer token weight.
///
/// This is a weight represented as (value / 1e18).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Weight(pub eth::U256);

impl Weight {
    fn base() -> eth::U256 {
        1_000_000_000_000_000_000_u128.into()
    }
}

impl From<eth::U256> for Weight {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

impl From<Weight> for eth::U256 {
    fn from(value: Weight) -> Self {
        value.0
    }
}

/// The weighted pool version. Different Balancer V2 weighted pool versions use
/// slightly different math.
#[derive(Clone, Copy, Debug)]
pub enum Version {
    /// Weighted pool math from the original Balancer V2 weighted pool
    /// implementation.
    V0,
    /// Weighted pool math for Balancer V2 weighted pools versions 3+. This uses
    /// a "shortcut" when computing exponentiation for 50/50 and 20/80 pools.
    V3Plus,
}
