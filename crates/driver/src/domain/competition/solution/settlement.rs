use crate::{
    boundary,
    domain::{competition, eth},
    Ethereum,
};

/// A transaction calling into our settlement contract on the blockchain.
///
/// Currently, this represents a wrapper around the [`boundary::Settlement`]
/// concept from the shared part of the codebase. This isn't well-defined
/// enough, it's an intermediate state between a solution and an onchain
/// settlement. The intention with this type is to represent the settlement
/// transaction itself, not an intermediate state.
#[derive(Debug, Clone)]
pub struct Settlement(boundary::Settlement);

impl Settlement {
    /// Encode a solution into an onchain settlement transaction.
    pub async fn encode(
        eth: &Ethereum,
        auction: &competition::Auction,
        solution: &competition::Solution,
    ) -> anyhow::Result<Self> {
        boundary::Settlement::encode(eth, solution, auction)
            .await
            .map(Self)
    }

    /// Calculate the score for this settlement. This method is here only
    /// temporarily, in the future the entire scoring formula should operate on
    /// a [`super::Solution`].
    pub async fn score(
        &self,
        eth: &Ethereum,
        auction: &competition::Auction,
        gas: eth::Gas,
    ) -> Result<super::Score, boundary::Error> {
        self.0.score(eth, auction, gas).await
    }

    // TODO Instead of tx, have a `From` impl, since this should really be a newtype
    // for eth::Tx
    /// The onchain transaction representing this settlement.
    pub fn tx(self) -> eth::Tx {
        self.0.tx()
    }
}
