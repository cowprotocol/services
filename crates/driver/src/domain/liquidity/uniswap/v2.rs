use {
    crate::{
        boundary,
        domain::{eth, liquidity},
    },
    std::cmp::Ordering,
};

/// The famous Uniswap V2 constant product pool, modelled by `x · y = k` [^1].
///
/// Note that there are many Uniswap V2 clones with identical behaviour from a
/// liquidity point of view, that are therefore modelled by the same type:
/// - SushiSwap (Mainnet and Gnosis Chain)
/// - Honeyswap (Gnosis Chain)
/// - Baoswap (Gnosis Chain)
///
/// [^1]: <https://uniswap.org/whitepaper.pdf>
#[derive(Clone, Debug)]
pub struct Pool {
    pub address: eth::Address,
    pub router: eth::ContractAddress,
    pub reserves: Reserves,
}

impl Pool {
    /// Encodes a pool swap as an interaction. Returns `Err` if the swap
    /// parameters are invalid for the pool, specifically if the input and
    /// output tokens don't correspond to the pool's token pair.
    pub fn swap(
        &self,
        input: &liquidity::MaxInput,
        output: &liquidity::ExactOutput,
        receiver: &eth::Address,
    ) -> Result<eth::Interaction, InvalidSwap> {
        if !self.reserves.has_tokens(&input.0.token, &output.0.token) {
            return Err(InvalidSwap);
        }

        Ok(boundary::liquidity::uniswap::v2::to_interaction(
            self, input, output, receiver,
        ))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("swap parameters do not match pool")]
pub struct InvalidSwap;

/// The reserves of a Uniswap V2 pool. These reserves are ordered by token
/// address and are guaranteed to be for distinct tokens.
#[derive(Clone, Copy, Debug)]
pub struct Reserves(eth::Asset, eth::Asset);

impl Reserves {
    /// Creates new Uniswap V2 token reserves, returns `Err` if the specified
    /// token addresses are equal.
    pub fn new(a: eth::Asset, b: eth::Asset) -> Result<Self, InvalidReserves> {
        match a.token.cmp(&b.token) {
            Ordering::Less => Ok(Self(a, b)),
            Ordering::Equal => Err(InvalidReserves),
            Ordering::Greater => Ok(Self(b, a)),
        }
    }

    /// Returns `true` if the reserves correspond to the specified tokens.
    fn has_tokens(&self, a: &eth::TokenAddress, b: &eth::TokenAddress) -> bool {
        (&self.0.token == a && &self.1.token == b) || (&self.1.token == a && &self.0.token == b)
    }

    /// Returns an iterator over the reserve assets.
    pub fn iter(&self) -> impl Iterator<Item = eth::Asset> {
        self.into_iter()
    }
}

impl IntoIterator for Reserves {
    type IntoIter = <[eth::Asset; 2] as IntoIterator>::IntoIter;
    type Item = eth::Asset;

    fn into_iter(self) -> Self::IntoIter {
        [self.0, self.1].into_iter()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid Uniswap V2 token reserves; assets cannot have the same token address")]
pub struct InvalidReserves;
