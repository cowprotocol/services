use crate::{
    api::{execute::ExecuteError, solve::SolveError},
    auction_converter::AuctionConverting,
    commit_reveal::{CommitRevealSolverAdapter, CommitRevealSolving, SettlementSummary},
};
use anyhow::{Context, Error, Result};
use futures::{StreamExt, TryFutureExt};
use gas_estimation::GasPriceEstimating;
use model::auction::AuctionWithId;
use primitive_types::H256;
use shared::current_block::{block_number, into_stream, CurrentBlockStream};
use solver::{
    driver::submit_settlement,
    driver_logger::DriverLogger,
    settlement::Settlement,
    settlement_rater::{SettlementRating, SimulationDetails},
    settlement_submission::{SolutionSubmitter, SubmissionError},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

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
        // TODO get deadline from autopilot auction
        let deadline = Instant::now() + Duration::from_secs(25);
        Self::solve_until_deadline(
            auction,
            self.solver.clone(),
            self.auction_converter.clone(),
            self.block_stream.clone(),
            deadline,
        )
        .await
        .map_err(SolveError::from)
    }

    /// Keeps solving the given auction with updated liquidity on every new block or until the
    /// auction deadline is reached.
    /// To get the most up to date solutions possible a task solving the auction gets spawned
    /// in the background as soon as a new block is detected. Background tasks can yield their
    /// results out of order but the method compensates for that fact. After the deadline has
    /// been reached the finished result belonging to the most recent block gets returned and
    /// unfinished tasks get aborted.
    async fn solve_until_deadline(
        auction: Auction,
        solver: Arc<dyn CommitRevealSolving>,
        converter: Arc<dyn AuctionConverting>,
        block_stream: CurrentBlockStream,
        deadline: Instant,
    ) -> Result<SettlementSummary> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(5);

        let start_computations = into_stream(block_stream).map(|latest_block| {
            let latest_block = match block_number(&latest_block) {
                Ok(number) => number,
                Err(_) => {
                    tracing::warn!(?latest_block, "new block doesn't have a block number");
                    return None;
                }
            };
            let solver = solver.clone();
            let converter = converter.clone();
            let auction = auction.clone();
            let tx = tx.clone();

            // spawn background task computing a solution for the new block.
            let task = tokio::task::spawn(async move {
                let result = converter
                    .convert_auction(auction, latest_block)
                    .and_then(|auction| solver.commit(auction))
                    .await;
                if let Err(err) = tx.send((latest_block, result)).await {
                    // Tasks get aborted before this channel gets closed when the deadline is
                    // reached. Because aborting the task might get delayed by the runtime this
                    // error could happen still. Even if the error happens it's not a big deal
                    // but let's keep an eye on it anyway.
                    tracing::debug!(?err, "result channel closed before solve task got aborted");
                }
            });
            Some(task)
        });

        let timeout = tokio::time::sleep_until(deadline.into());
        tokio::pin!(timeout, start_computations);
        let mut spawned_tasks = vec![];
        let mut most_recent_solution = None;

        loop {
            tokio::select! {
                // keep the `JoinHandle`s to abort unfinished tasks when the deadline is reached
                task = start_computations.next() => spawned_tasks.extend(task.flatten()),
                new_solution = rx.recv() => {
                    most_recent_solution = match (new_solution, most_recent_solution.take()) {
                        // keep new result because any result is better than no result
                        (Some(new_solution), None) => Some(new_solution),
                        // new result was computed from a later block than current result
                        (Some((new_block, result)), Some((current_block, _))) if new_block > current_block => {
                            Some((new_block, result))
                        }
                        // stream ended or produced an outdated result
                        (_, most_recent_solution) => most_recent_solution
                    };
                },
                _ = &mut timeout => {
                    // abort computation tasks (aborting a finished task is a no-op)
                    spawned_tasks.iter().for_each(|task| task.abort());
                    return match most_recent_solution.take() {
                        None => Err(anyhow::anyhow!("could not compute a result before the deadline")),
                        Some((_block, result)) => result
                    }
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auction_converter::MockAuctionConverting, commit_reveal::MockCommitRevealSolving};
    use futures::FutureExt;
    use shared::current_block::Block;
    use std::sync::Arc;
    use tokio::{sync::watch::channel, time::sleep};

    fn block(number: Option<u64>) -> Block {
        Block {
            number: number.map(|n| n.into()),
            ..Default::default()
        }
    }

    fn deadline(milliseconds_from_now: u64) -> Instant {
        Instant::now() + Duration::from_millis(milliseconds_from_now)
    }

    #[tokio::test]
    async fn no_block_number_means_no_computation() {
        let (_tx, rx) = channel(block(None));
        let converter = MockAuctionConverting::new();
        let solver = MockCommitRevealSolving::new();
        let result = Driver::solve_until_deadline(
            Default::default(),
            Arc::new(solver),
            Arc::new(converter),
            rx.clone(),
            deadline(10),
        )
        .await;

        assert_eq!(
            result.unwrap_err().to_string(),
            "could not compute a result before the deadline"
        );
    }

    #[tokio::test]
    async fn propagates_error_from_auction_conversion() {
        let (_tx, rx) = channel(block(Some(1)));
        let mut converter = MockAuctionConverting::new();
        converter
            .expect_convert_auction()
            .returning(|_, _| async { anyhow::bail!("failed to convert auction") }.boxed());
        let solver = MockCommitRevealSolving::new();
        let result = Driver::solve_until_deadline(
            Default::default(),
            Arc::new(solver),
            Arc::new(converter),
            rx.clone(),
            deadline(10),
        )
        .await;

        assert_eq!(result.unwrap_err().to_string(), "failed to convert auction");
    }

    #[tokio::test]
    async fn propagates_error_from_auction_solving() {
        let (_tx, rx) = channel(block(Some(1)));
        let mut converter = MockAuctionConverting::new();
        converter
            .expect_convert_auction()
            .returning(|_, _| async { Ok(Default::default()) }.boxed());
        let mut solver = MockCommitRevealSolving::new();
        solver
            .expect_commit()
            .returning(|_| async { Err(anyhow::anyhow!("failed to solve auction")) }.boxed());
        let result = Driver::solve_until_deadline(
            Default::default(),
            Arc::new(solver),
            Arc::new(converter),
            rx.clone(),
            deadline(10),
        )
        .await;

        assert_eq!(result.unwrap_err().to_string(), "failed to solve auction");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn solution_for_last_block_wins_when_it_was_computed_last() {
        let (tx, rx) = channel(block(Some(1)));
        let mut converter = MockAuctionConverting::new();
        converter
            .expect_convert_auction()
            .returning(|_, block| {
                async move {
                    Ok(solver::solver::Auction {
                        liquidity_fetch_block: block,
                        ..Default::default()
                    })
                }
                .boxed()
            })
            .times(2);

        let mut solver = MockCommitRevealSolving::new();
        solver
            .expect_commit()
            .return_once(move |auction| {
                assert_eq!(auction.liquidity_fetch_block, 1);
                async move {
                    // there is no great place to trigger the next block so let's do it here
                    tx.send(block(Some(2))).unwrap();
                    anyhow::bail!("failed to solve auction")
                }
                .boxed()
            })
            .times(1);
        solver
            .expect_commit()
            .returning(|auction| {
                assert_eq!(auction.liquidity_fetch_block, 2);
                async { Ok(Default::default()) }.boxed()
            })
            .times(1);

        let result = Driver::solve_until_deadline(
            Default::default(),
            Arc::new(solver),
            Arc::new(converter),
            rx.clone(),
            deadline(10),
        )
        .await
        .unwrap();
        assert_eq!(result, SettlementSummary::default());
    }

    #[tokio::test]
    async fn solution_for_last_block_wins_when_it_was_computed_first() {
        let (tx, rx) = channel(block(Some(1)));
        let mut converter = MockAuctionConverting::new();
        converter
            .expect_convert_auction()
            .returning(|_, block| {
                async move {
                    Ok(solver::solver::Auction {
                        liquidity_fetch_block: block,
                        ..Default::default()
                    })
                }
                .boxed()
            })
            .times(2);

        let mut solver = MockCommitRevealSolving::new();
        solver
            .expect_commit()
            .return_once(move |auction| {
                assert_eq!(auction.liquidity_fetch_block, 1);
                async move {
                    // there is no great place to trigger the next block so let's do it here
                    tx.send(block(Some(2))).unwrap();
                    sleep(tokio::time::Duration::from_millis(100)).await;
                    anyhow::bail!("failed to solve auction")
                }
                .boxed()
            })
            .times(1);
        solver
            .expect_commit()
            .returning(|auction| {
                assert_eq!(auction.liquidity_fetch_block, 2);
                async { Ok(Default::default()) }.boxed()
            })
            .times(1);

        let result = Driver::solve_until_deadline(
            Default::default(),
            Arc::new(solver),
            Arc::new(converter),
            rx.clone(),
            deadline(10),
        )
        .await
        .unwrap();
        assert_eq!(result, SettlementSummary::default());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn solving_next_block_can_start_before_finishing_previous_block() {
        let (tx, rx) = channel(block(Some(1)));
        let mut converter = MockAuctionConverting::new();
        converter
            .expect_convert_auction()
            .return_once(move |_, block_| {
                async move {
                    tx.send(block(Some(2))).unwrap();
                    sleep(tokio::time::Duration::from_millis(50)).await;
                    Ok(solver::solver::Auction {
                        liquidity_fetch_block: block_,
                        ..Default::default()
                    })
                }
                .boxed()
            })
            .times(1);
        converter
            .expect_convert_auction()
            .return_once(move |_, block_| {
                async move {
                    Ok(solver::solver::Auction {
                        liquidity_fetch_block: block_,
                        ..Default::default()
                    })
                }
                .boxed()
            })
            .times(1);

        let mut solver = MockCommitRevealSolving::new();
        solver
            .expect_commit()
            .return_once(move |auction| {
                // start solving first block first because new block appeared before
                // auction for first block was prepared
                assert_eq!(auction.liquidity_fetch_block, 2);
                async move {
                    sleep(tokio::time::Duration::from_millis(100)).await;
                    anyhow::bail!("failed to solve auction")
                }
                .boxed()
            })
            .times(1);
        solver
            .expect_commit()
            .returning(move |auction| {
                assert_eq!(auction.liquidity_fetch_block, 1);
                async { Ok(Default::default()) }.boxed()
            })
            .times(1);

        let result = Driver::solve_until_deadline(
            Default::default(),
            Arc::new(solver),
            Arc::new(converter),
            rx.clone(),
            deadline(100),
        )
        .await
        .unwrap();
        assert_eq!(result, SettlementSummary::default());
    }
}
