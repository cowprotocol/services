use crate::{
    boundary,
    logic::{competition, eth},
    Ethereum,
    Solver,
};

/// A transaction calling into our settlement contract on the blockchain.
///
/// Currently, this represents a wrapper around the [`boundary::Settlement`]
/// concept from the shared part of the codebase. This isn't a well-defined
/// enough, it's an intermediate state between a solution and an onchain
/// settlement. The intention with this type is to represent the settlement
/// transaction itself, not an intermediate state.
#[derive(Debug)]
pub struct Settlement(boundary::Settlement);

impl Settlement {
    /// Encode a solution into an onchain settlement transaction.
    pub async fn encode(
        solver: &Solver,
        eth: &Ethereum,
        auction: &competition::Auction,
        solution: competition::Solution,
    ) -> anyhow::Result<Self> {
        boundary::Settlement::encode(eth, solver, solution, auction)
            .await
            .map(Self)
    }

    /// Calculate the score for this settlement. This method is here only
    /// temporarily, in the future the entire scoring formula should operate on
    /// a [`super::Solution`].
    pub(super) async fn score(
        self,
        eth: &Ethereum,
        auction: &competition::Auction,
        gas: eth::Gas,
    ) -> Result<super::Score, boundary::Error> {
        self.0.score(eth, auction, gas).await
    }
}
