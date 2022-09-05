use crate::{
    api::{execute::ExecuteError, solve::SolveError},
    auction_converter::AuctionConverting,
    commit_reveal::{CommitRevealSolverAdapter, CommitRevealSolving, SettlementSummary},
};
use anyhow::{Context, Error, Result};
use futures::{
    future::FutureExt as _,
    {stream::Stream, StreamExt},
};
use gas_estimation::GasPriceEstimating;
use model::auction::AuctionWithId;
use primitive_types::H256;
use shared::current_block::{block_number, into_stream, Block, CurrentBlockStream};
use solver::{
    driver::submit_settlement,
    driver_logger::DriverLogger,
    settlement::Settlement,
    settlement_rater::{SettlementRating, SimulationDetails},
    settlement_submission::{SolutionSubmitter, SubmissionError},
};
use std::{future::Future, pin::Pin, sync::Arc};

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
        auction: AuctionWithId,
    ) -> Result<SettlementSummary, SolveError> {
        self.solve_until_deadline(auction)
            .await
            .map_err(SolveError::from)
    }

    /// Computes a solution with the liquidity collected from a given block.
    async fn compute_solution_for_block(
        auction: Auction,
        block: Block,
        converter: Arc<dyn AuctionConverting>,
        solver: Arc<dyn CommitRevealSolving>,
    ) -> Result<SettlementSummary> {
        let block = block_number(&block)?;
        let auction = converter.convert_auction(auction, block).await?;
        solver.commit(auction).await
    }

    /// Keeps solving the given auction with the most recent liquidity at that time or until the
    /// auction deadline is reached.
    async fn solve_until_deadline(&self, auction: Auction) -> Result<SettlementSummary> {
        // TODO get deadline from autopilot auction
        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(25));
        let block_stream = into_stream(self.block_stream.clone());
        last_completed(
            |block| {
                Self::compute_solution_for_block(
                    auction.clone(),
                    block,
                    self.auction_converter.clone(),
                    self.solver.clone(),
                )
                .boxed()
            },
            block_stream,
            timeout,
        )
        .await
        .unwrap_or_else(|| Err(anyhow::anyhow!("could not compute a solution in time")))
    }

    /// Validates that the `Settlement` satisfies expected fairness and correctness properties.
    async fn validate_settlement(&self, settlement: Settlement) -> Result<SimulationDetails> {
        let gas_price = self.gas_price_estimator.estimate().await?;
        let fake_solver = Arc::new(CommitRevealSolverAdapter::from(self.solver.clone()));
        let simulation_details = self
            .settlement_rater
            .simulate_settlements(vec![(fake_solver, settlement)], gas_price)
            .await?
            .pop()
            .context("simulation returned no results")?;
        match simulation_details.gas_estimate {
            Err(err) => return Err(Error::from(err)).context("simulation failed"),
            Ok(gas_estimate) => tracing::info!(?gas_estimate, "settlement simulated successfully"),
        }
        Ok(simulation_details)
    }

    /// When the solver won the competition it finalizes the `Settlement` and decides whether it
    /// still wants to execute and submit that `Settlement`.
    pub async fn on_auction_won(&self, summary: SettlementSummary) -> Result<H256, ExecuteError> {
        tracing::info!("solver won the auction");
        let settlement = match self.solver.reveal(&summary).await? {
            None => {
                tracing::info!("solver decided against executing the settlement");
                return Err(ExecuteError::ExecutionRejected);
            }
            Some(solution) => solution,
        };
        tracing::info!(?settlement, "received settlement from solver");
        let simulation_details = self.validate_settlement(settlement).await?;
        self.submit_settlement(simulation_details)
            .await
            // TODO correctly propagate specific errors to the end
            .map_err(|e| ExecuteError::from(e.into_anyhow()))
    }

    /// Tries to submit the `Settlement` on chain. Returns a transaction hash if it was successful.
    async fn submit_settlement(
        &self,
        simulation_details: SimulationDetails,
    ) -> Result<H256, SubmissionError> {
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
            None, // the concept of a settlement_id does not make sense here
        )
        .await
        .map(|receipt| receipt.transaction_hash)
    }
}

/// Polls the `producer` for new work and buffers only the most recent one. Whenever a
/// computational task finishes a new one will be started with the most recently buffered work.
/// Returns the most recent output from a computational task when the `deadline` has been reached.
async fn last_completed<T, W, B, P, D>(build_task: B, producer: P, deadline: D) -> Option<T>
where
    B: Fn(W) -> Pin<Box<dyn Future<Output = T> + Send>>,
    P: Stream<Item = W>,
    D: Future<Output = ()>,
    T: Send,
    W: Send,
{
    futures::pin_mut!(producer);
    futures::pin_mut!(deadline);

    let mut result = None;
    let mut next_work = None;

    let mut currently_computing = false;
    // initialize with future that does nothing and can be dropped safely
    let mut current_task = futures::future::pending().fuse().boxed();

    loop {
        tokio::select! {
            r = &mut current_task => {
                result = Some(r);
                if let Some(work) = next_work.take() {
                    current_task = build_task(work);
                }
            }
            new_work = producer.next() => {
                match (new_work, currently_computing) {
                    (Some(work), true) => {
                        next_work = Some(work);
                    }
                    (Some(work), false) => {
                        current_task = build_task(work);
                        currently_computing = true;
                    }
                    // stream terminated and there is no compuration going on => return early
                    (None, true) => break,
                    // stream terminated but we might still get a result from the current
                    // computation => do nothing
                    (None, false) => ()
                };
            }
            _ = &mut deadline => break,
        }
    }
    result
}
