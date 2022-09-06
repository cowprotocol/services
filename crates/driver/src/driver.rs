use crate::{
    api::{execute::ExecuteError, solve::SolveError},
    auction_converter::AuctionConverting,
    commit_reveal::{CommitRevealSolverAdapter, CommitRevealSolving, SettlementSummary},
};
use anyhow::{Context, Error, Result};
use futures::{
    future::{FusedFuture, FutureExt},
    stream::{FusedStream, StreamExt},
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
use std::{
    pin::Pin,
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
        Self::solve_until_deadline(
            auction,
            self.solver.clone(),
            self.auction_converter.clone(),
            self.block_stream.clone(),
            // TODO get deadline from autopilot auction
            Instant::now() + Duration::from_secs(25),
        )
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
    async fn solve_until_deadline(
        auction: Auction,
        solver: Arc<dyn CommitRevealSolving>,
        converter: Arc<dyn AuctionConverting>,
        block_stream: CurrentBlockStream,
        deadline: Instant,
    ) -> Result<SettlementSummary> {
        last_completed(
            |block| {
                Box::pin(
                    Self::compute_solution_for_block(
                        auction.clone(),
                        block,
                        converter.clone(),
                        solver.clone(),
                    )
                    .fuse(),
                )
            },
            into_stream(block_stream).fuse(),
            tokio::time::sleep_until(deadline.into()).fuse(),
        )
        .await
        .unwrap_or_else(|| {
            Err(anyhow::anyhow!(
                "could not compute a result before the deadline"
            ))
        })
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
    B: Fn(W) -> Pin<Box<dyn FusedFuture<Output = T> + Send>>,
    P: FusedStream<Item = W>,
    D: FusedFuture<Output = ()>,
    T: Send,
    W: Send,
{
    futures::pin_mut!(producer);
    futures::pin_mut!(deadline);

    let mut result = None;
    let mut next_work = None;

    let create_pending_future = || {
        Box::pin(futures::future::pending::<T>().fuse())
            as Pin<Box<dyn FusedFuture<Output = _> + Send>>
    };

    let mut currently_computing = false;
    // initialize with future that does nothing and can be dropped safely
    let mut current_task = create_pending_future();

    loop {
        futures::select_biased! {
            _ = &mut deadline => {
                println!("deadline reached");
                break
            }
            new_work = producer.next() => {
                match (new_work, currently_computing) {
                    (Some(new_work), true) => next_work = Some(new_work),
                    (Some(new_work), false) => current_task = build_task(new_work),
                    (None, false) => break,
                    (None, true) => ()
                };
                // either we are currently computing or we created a new task to work on
                currently_computing = true;
            }
            r = &mut current_task => {
                result = Some(r);
                let (new_task, is_computing) = match (next_work.take(), producer.is_terminated()) {
                    // start on the buffered work
                    (Some(work), _) => (build_task(work), true),
                    // wait for producer to give us more work
                    (None, false) => (create_pending_future(), false),
                    // no work left and producer terminated => return early
                    (None, true) => break,
                };
                current_task = new_task;
                currently_computing = is_computing;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auction_converter::MockAuctionConverting, commit_reveal::MockCommitRevealSolving};
    use futures::FutureExt;
    use shared::current_block::Block;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
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
    async fn no_block_number_results_in_error() {
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

        assert_eq!(result.unwrap_err().to_string(), "no block number");
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

    #[tokio::test]
    async fn follow_up_computations_use_the_latest_block() {
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
                    tx.send(block(Some(3))).unwrap();
                    // yield this thread such that the block stream can see the new blocks
                    sleep(Duration::from_millis(10)).await;
                    anyhow::bail!("failed to solve auction")
                }
                .boxed()
            })
            .times(1);
        solver
            .expect_commit()
            .returning(|auction| {
                assert_eq!(auction.liquidity_fetch_block, 3);
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

    #[tokio::test]
    async fn first_computation_starts_with_the_latest_block() {
        let (tx, rx) = channel(block(Some(1)));
        tx.send(block(Some(2))).unwrap();
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
            .times(1);

        let mut solver = MockCommitRevealSolving::new();
        solver
            .expect_commit()
            .return_once(move |auction| {
                assert_eq!(auction.liquidity_fetch_block, 2);
                async move { Ok(Default::default()) }.boxed()
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
    async fn solving_can_end_early_when_stream_terminates() {
        let start = Instant::now();
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
            .times(1);

        let mut solver = MockCommitRevealSolving::new();
        solver
            .expect_commit()
            .return_once(move |auction| {
                assert_eq!(auction.liquidity_fetch_block, 1);
                // drop sender to terminate the block stream while computing a result
                drop(tx);
                async move { Ok(Default::default()) }.boxed()
            })
            .times(1);

        let result = Driver::solve_until_deadline(
            Default::default(),
            Arc::new(solver),
            Arc::new(converter),
            rx.clone(),
            deadline(1_000),
        )
        .await
        .unwrap();
        assert_eq!(result, SettlementSummary::default());
        assert!(start.elapsed().as_millis() < 100);
    }
}
