use crate::domain::{eth, liquidity};

/// Liquidity data tied to a Balancer V2 stable pool.
///
/// These pools are an implementation of Curve.fi StableSwap pools [^1] on the
/// Balancer V2 Vault contract [^2].
///
/// [^1]: <https://classic.curve.fi/whitepaper>
/// [^2]: <https://docs.balancer.fi/products/balancer-pools/composable-stable-pools>
#[derive(Clone, Debug)]
pub struct Pool {}

impl Pool {
    /// Encodes a pool swap as an interaction. Returns `Err` if the swap
    /// parameters are invalid for the pool, specifically if the input and
    /// output tokens do not belong to the pool.
    pub fn swap(
        &self,
        _input: &liquidity::MaxInput,
        _output: &liquidity::ExactOutput,
        _receiver: &eth::Address,
    ) -> Result<eth::Interaction, liquidity::InvalidSwap> {
        todo!()
    }
}
