use {
    super::{Fee, Id, ScalingFactor},
    crate::{
        boundary,
        domain::{eth, liquidity},
    },
    itertools::Itertools,
};

/// Liquidity data tied to a Balancer V2 stable pool.
///
/// These pools are an implementation of Curve.fi StableSwap pools [^1] on the
/// Balancer V2 Vault contract [^2].
///
/// [^1]: <https://classic.curve.fi/whitepaper>
/// [^2]: <https://docs.balancer.fi/products/balancer-pools/composable-stable-pools>
#[derive(Clone, Debug)]
pub struct Pool {
    pub vault: eth::ContractAddress,
    pub id: Id,
    pub reserves: Reserves,
    pub amplification_parameter: AmplificationParameter,
    pub fee: Fee,
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

        Ok(boundary::liquidity::balancer::v2::stable::to_interaction(
            self, input, output, receiver,
        ))
    }
}

/// Balancer stable pool reserves.
///
/// This is an ordered collection of tokens with their balance and scaling
/// factors.
#[derive(Clone, Debug)]
pub struct Reserves(Vec<Reserve>);

impl Reserves {
    /// Creates new Balancer V2 token reserves, returns `Err` if the specified
    /// token reserves are invalid, specifically, if there are duplicate tokens.
    pub fn new(reserves: Vec<Reserve>) -> Result<Self, InvalidReserves> {
        if !reserves.iter().map(|r| r.asset.token).all_unique() {
            return Err(InvalidReserves);
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
#[error("invalid Balancer V2 token reserves; duplicate token address")]
pub struct InvalidReserves;

/// Balancer weighted pool reserve for a single token.
#[derive(Clone, Copy, Debug)]
pub struct Reserve {
    pub asset: eth::Asset,
    pub scale: ScalingFactor,
}

/// Balancer V2 stable pool amplification parameter.
///
/// Internally, this is represented as a ratio of [`eth::U256`]s.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmplificationParameter {
    factor: eth::U256,
    precision: eth::U256,
}

impl AmplificationParameter {
    pub fn new(
        factor: eth::U256,
        precision: eth::U256,
    ) -> Result<Self, InvalidAmplificationParameter> {
        if precision.is_zero() {
            return Err(InvalidAmplificationParameter::ZeroDenominator);
        }

        if factor.overflowing_mul(precision).1 {
            return Err(InvalidAmplificationParameter::Overflow);
        }

        Ok(Self { factor, precision })
    }

    pub fn factor(&self) -> eth::U256 {
        self.factor
    }

    pub fn precision(&self) -> eth::U256 {
        self.precision
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidAmplificationParameter {
    #[error("invalid Balancer V2 amplification parameter; 0 denominator")]
    ZeroDenominator,

    #[error("invalid Balancer V2 amplification parameter; overflow U256 representation")]
    Overflow,
}
