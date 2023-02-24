use crate::{
    boundary,
    domain::{eth, liquidity},
};

/// Liquidity data tied to a Balancer V2 pool based on "Weighted Math" [^1].
/// as a generalization of constant product liquidity pools (x * y = k)
/// that allow for more than 2 tokens and arbitrary weighting of each token.
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
    pub id: Id,
    pub tokens: Vec<TokenState>,
    pub swap_fee: Fee,
    pub vault: eth::ContractAddress,
}

#[derive(Debug, Clone)]
pub struct TokenState {
    pub weight: Weight,
    pub asset: eth::Asset,
    pub scaling_exponent: u8,
}

/// Fixed point numbers that represent exactly any rational number that can be
/// represented with up to 18 decimals as long as it can be stored in 256 bits.
/// It corresponds to Solidity's `ufixed256x18`.
/// Operations on this type are implemented as in Balancer's FixedPoint library.
#[derive(Debug, Clone)]
pub struct Bfp(pub eth::U256);

/// Fee taken when swapping with this pool.
#[derive(Debug, Clone)]
pub struct Fee(pub Bfp);

/// Unique identifier of a liquidity pool.
#[derive(Debug, Clone)]
pub struct Id(pub eth::H256);

/// How much the associated token contributes to the overall value in the pool.
#[derive(Debug, Clone)]
pub struct Weight(pub Bfp);

impl Pool {
    /// Encodes a pool swap as an interaction. Returns `None` if the swap
    /// parameters are invalid for the pool, specifically if the input and
    /// output tokens don't correspond to the pool's token pair.
    pub fn swap(
        &self,
        input: &liquidity::MaxInput,
        output: &liquidity::ExactOutput,
        receiver: &eth::Address,
    ) -> Option<eth::Interaction> {
        let tokens_match = [input.0.token, output.0.token]
            .iter()
            .all(|token| self.tokens.iter().any(|state| state.asset.token == *token));

        tokens_match.then_some(boundary::liquidity::balancer::weighted::to_interaction(
            self, input, output, receiver,
        ))
    }
}
