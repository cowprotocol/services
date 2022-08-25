use crate::{
    api::{execute::ExecuteError, solve::SolveError},
    auction_converter::AuctionConverting,
    commit_reveal::{CommitRevealSolverAdapter, CommitRevealSolving, SettlementSummary},
};
use anyhow::{Context, Result};
use gas_estimation::GasPriceEstimating;
use model::auction::Auction;
use shared::current_block::{block_number, CurrentBlockStream};
use solver::{
    driver::submit_settlement,
    driver_logger::DriverLogger,
    settlement::Settlement,
    settlement_rater::{SettlementRating, SimulationDetails},
    settlement_submission::{SolutionSubmitter, SubmissionError},
};
use std::sync::Arc;
use web3::types::TransactionReceipt;

pub struct Driver {
    pub solver: Arc<dyn CommitRevealSolving>,
    pub submitter: Arc<SolutionSubmitter>,
    pub auction_converter: Arc<dyn AuctionConverting>,
    pub block_stream: CurrentBlockStream,
    pub settlement_rater: Arc<dyn SettlementRating>,
    pub logger: Arc<DriverLogger>,
    pub gas_price_estimator: Arc<dyn GasPriceEstimating>,
}

impl Driver {
    /// Does some sanity checks on the auction, collects some liquidity and prepares the auction
    /// for the solver.
    pub async fn on_auction_started(
        &self,
        auction: Auction,
    ) -> Result<SettlementSummary, SolveError> {
        tracing::info!(?auction, "received new auction");
        let fetch_liquidity_from_block = block_number(&self.block_stream.borrow())?;
        let auction = self
            .auction_converter
            .convert_auction(auction, fetch_liquidity_from_block)
            .await?;
        tracing::debug!(?auction, "converted original auction to useful type");
        self.solver.commit(auction).await.map_err(SolveError::from)
    }

    /// Validates that the `Settlement` satisfies expected fairness and correctness properties.
    async fn validate_settlement(&self, settlement: Settlement) -> Result<SimulationDetails> {
        let gas_price = self.gas_price_estimator.estimate().await?;
        let fake_solver = Arc::new(CommitRevealSolverAdapter::from(self.solver.clone()));
        tracing::debug!(?gas_price, ?settlement, "simulating settlement");
        let simulation_details = self
            .settlement_rater
            .simulate_settlements(vec![(fake_solver, settlement)], gas_price)
            .await?
            .pop()
            .context("simulation returned no results")?;
        anyhow::ensure!(
            simulation_details.gas_estimate.is_ok(),
            "settlement reverted during simulation"
        );
        Ok(simulation_details)
    }

    /// When the solver won the competition it finalizes the `Settlement` and decides whether it
    /// still wants to execute and submit that `Settlement`.
    pub async fn on_auction_won(
        &self,
        summary: SettlementSummary,
    ) -> Result<TransactionReceipt, ExecuteError> {
        let settlement = match self.solver.reveal(&summary).await? {
            None => {
                tracing::info!("solver decided against executing the settlement");
                return Err(ExecuteError::ExecutionRejected);
            }
            Some(solution) => solution,
        };
        let simulation_details = self.validate_settlement(settlement).await?;
        self.submit_settlement(simulation_details, summary.settlement_id)
            .await
            // TODO correctly propagate specific errors to the end
            .map_err(|e| ExecuteError::from(e.into_anyhow()))
    }

    /// Tries to submit the `Settlement` on chain. Returns a transaction hash if it was successful.
    async fn submit_settlement(
        &self,
        simulation_details: SimulationDetails,
        settlement_id: u64,
    ) -> Result<TransactionReceipt, SubmissionError> {
        let gas_estimate = simulation_details
            .gas_estimate
            .expect("checked simulation gas_estimate during validation");
        tracing::info!(?gas_estimate, settlement =? simulation_details.settlement, "start submitting settlement");
        submit_settlement(
            &self.submitter,
            &self.logger,
            simulation_details.solver,
            simulation_details.settlement,
            gas_estimate,
            settlement_id,
        )
        .await
    }
}
