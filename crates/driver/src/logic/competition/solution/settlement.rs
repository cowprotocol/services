use crate::{boundary, logic::competition, Ethereum, Simulator, Solver};

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
        boundary::settlement::encode(eth, solver, solution, auction)
            .await
            .map(Self)
    }

    /// Calculate the score for this settlement. This method is here only
    /// temporarily, in the future the entire scoring formula should operate on
    /// a [`super::Solution`].
    pub(super) fn score(&self, _simulator: &Simulator) -> super::Score {
        // TODO This will also call into the boundary because the objective value
        // calculation is tricky and difficult to get right. This is a short-term
        // solution, I'd like to revisit that logic because it seems a bit convoluted
        // and I wonder if we can make it correspond more closely to the descriptions
        // and formulas that we have on docs.cow.fi
        //
        // TODO I intend to do the access list generation and gas estimation in driver
        // though, that will not be part of the boundary
        todo!()
    }
}
