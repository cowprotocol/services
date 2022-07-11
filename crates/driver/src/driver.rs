use crate::{
    api::{execute::ExecuteError, solve::SolveError},
    commit_reveal::{CommitRevealSolving, SettlementSummary},
};
use anyhow::Result;
use model::auction::Auction;
use solver::{settlement::Settlement, settlement_submission::SolutionSubmitter};
use std::sync::Arc;

pub struct Driver {
    pub solver: Arc<dyn CommitRevealSolving>,
    pub submitter: Arc<SolutionSubmitter>,
}

impl Driver {
    /// Does some sanity checks on the auction, collects some liquidity and prepares the auction
    /// for the solver.
    pub async fn on_auction_started(
        &self,
        _auction: Auction,
    ) -> Result<SettlementSummary, SolveError> {
        // TODO sanity checks
        // TODO liquidity collection
        let auction = solver::solver::Auction::default();
        self.solver.commit(auction).await.map_err(SolveError::from)
    }

    /// Validates that the `Settlement` satisfies expected fairness and correctness properties.
    async fn validate_settlement(&self, _settlement: &Settlement) -> Result<()> {
        // TODO simulation
        // TODO token conservation
        Ok(())
    }

    /// When the solver won the competition it finalizes the `Settlement` and decides whether it
    /// still wants to execute and submit that `Settlement`.
    pub async fn on_auction_won(&self, summary: SettlementSummary) -> Result<(), ExecuteError> {
        let settlement = match self.solver.reveal(summary).await? {
            None => return Err(ExecuteError::ExecutionRejected),
            Some(solution) => solution,
        };
        self.validate_settlement(&settlement).await?;
        self.submit_settlement(settlement).await?;
        Ok(())
    }

    /// Tries to submit the `Settlement` on chain. Returns a transaction hash if it was successful.
    async fn submit_settlement(&self, _settlement: Settlement) -> Result<()> {
        // TODO execute
        // TODO notify about execution
        Ok(())
    }
}
