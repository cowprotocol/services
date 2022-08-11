pub mod solver_settlements;

use self::solver_settlements::RatedSettlement;
use crate::{
    auction_preprocessing,
    driver_logger::DriverLogger,
    in_flight_orders::InFlightOrders,
    liquidity::order_converter::OrderConverter,
    liquidity_collector::{LiquidityCollecting, LiquidityCollector},
    metrics::SolverMetrics,
    orderbook::OrderBookApi,
    settlement::{external_prices::ExternalPrices, PriceCheckTokens, Settlement},
    settlement_post_processing::PostProcessingPipeline,
    settlement_ranker::SettlementRanker,
    settlement_rater::SettlementRater,
    settlement_simulation::{self, TenderlyApi},
    settlement_submission::{SolutionSubmitter, SubmissionError},
    solver::{Auction, Solver, SolverRunError, Solvers},
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use model::solver_competition::CompetitionAuction;
use model::solver_competition::{
    self, Objective, SolverCompetition, SolverCompetitionId, SolverSettlement,
};
use num::{rational::Ratio, BigInt, BigRational, ToPrimitive};
use primitive_types::H160;
use shared::{
    current_block::{self, CurrentBlockStream},
    recent_block_cache::Block,
    token_list::TokenList,
    Web3,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::Instrument as _;
use web3::types::TransactionReceipt;

pub struct Driver {
    liquidity_collector: LiquidityCollector,
    solvers: Solvers,
    gas_price_estimator: Arc<dyn GasPriceEstimating>,
    settle_interval: Duration,
    native_token: H160,
    metrics: Arc<dyn SolverMetrics>,
    solver_time_limit: Duration,
    block_stream: CurrentBlockStream,
    solution_submitter: SolutionSubmitter,
    run_id: u64,
    api: OrderBookApi,
    order_converter: Arc<OrderConverter>,
    in_flight_orders: InFlightOrders,
    post_processing_pipeline: PostProcessingPipeline,
    fee_objective_scaling_factor: BigRational,
    settlement_ranker: SettlementRanker,
    logger: DriverLogger,
}
impl Driver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settlement_contract: GPv2Settlement,
        liquidity_collector: LiquidityCollector,
        solvers: Solvers,
        gas_price_estimator: Arc<dyn GasPriceEstimating>,
        settle_interval: Duration,
        native_token: H160,
        min_order_age: Duration,
        metrics: Arc<dyn SolverMetrics>,
        web3: Web3,
        network_id: String,
        solver_time_limit: Duration,
        market_makable_token_list: Option<TokenList>,
        block_stream: CurrentBlockStream,
        solution_submitter: SolutionSubmitter,
        api: OrderBookApi,
        order_converter: Arc<OrderConverter>,
        weth_unwrap_factor: f64,
        simulation_gas_limit: u128,
        fee_objective_scaling_factor: f64,
        max_settlement_price_deviation: Option<Ratio<BigInt>>,
        token_list_restriction_for_price_checks: PriceCheckTokens,
        tenderly: Option<TenderlyApi>,
    ) -> Self {
        let post_processing_pipeline = PostProcessingPipeline::new(
            native_token,
            web3.clone(),
            weth_unwrap_factor,
            settlement_contract.clone(),
            market_makable_token_list,
        );

        let settlement_rater = Box::new(SettlementRater {
            access_list_estimator: solution_submitter.access_list_estimator.clone(),
            settlement_contract: settlement_contract.clone(),
            web3: web3.clone(),
        });

        let settlement_ranker = SettlementRanker {
            max_settlement_price_deviation,
            token_list_restriction_for_price_checks,
            metrics: metrics.clone(),
            min_order_age,
            settlement_rater,
        };

        let logger = DriverLogger {
            metrics: metrics.clone(),
            web3,
            tenderly,
            network_id,
            settlement_contract,
            simulation_gas_limit,
        };

        Self {
            liquidity_collector,
            solvers,
            gas_price_estimator,
            settle_interval,
            native_token,
            metrics,
            solver_time_limit,
            block_stream,
            solution_submitter,
            run_id: 0,
            api,
            order_converter,
            in_flight_orders: InFlightOrders::default(),
            post_processing_pipeline,
            fee_objective_scaling_factor: BigRational::from_float(fee_objective_scaling_factor)
                .unwrap(),
            settlement_ranker,
            logger,
        }
    }

    pub async fn run_forever(&mut self) -> ! {
        loop {
            match self.single_run().await {
                Ok(()) => tracing::debug!("single run finished ok"),
                Err(err) => tracing::error!("single run errored: {:?}", err),
            }
            self.metrics.runloop_completed();
            tokio::time::sleep(self.settle_interval).await;
        }
    }

    // Returns solver name and result.
    async fn run_solvers(
        &self,
        auction: Auction,
    ) -> Vec<(Arc<dyn Solver>, Result<Vec<Settlement>, SolverRunError>)> {
        join_all(self.solvers.iter().map(|solver| {
            let auction = auction.clone();
            let metrics = &self.metrics;
            async move {
                let start_time = Instant::now();
                let result =
                    match tokio::time::timeout_at(auction.deadline.into(), solver.solve(auction))
                        .await
                    {
                        Ok(inner) => inner.map_err(SolverRunError::Solving),
                        Err(_timeout) => Err(SolverRunError::Timeout),
                    };
                metrics.settlement_computed(solver.name(), start_time);
                (solver.clone(), result)
            }
        }))
        .await
    }

    pub async fn single_run(&mut self) -> Result<()> {
        let auction = self
            .api
            .get_auction()
            .await
            .context("error retrieving current auction")?;

        let id = auction.next_solver_competition;
        let run = self.next_run_id();

        // extra function so that we can add span information
        self.single_auction(auction, run)
            .instrument(tracing::info_span!("auction", id, run))
            .await
    }

    async fn single_auction(
        &mut self,
        mut auction: model::auction::Auction,
        run_id: u64,
    ) -> Result<()> {
        let start = Instant::now();
        tracing::debug!("starting single run");

        let current_block_during_liquidity_fetch =
            current_block::block_number(&self.block_stream.borrow())?;

        let before_count = auction.orders.len();
        let inflight_order_uids = self.in_flight_orders.update_and_filter(&mut auction);
        if before_count != auction.orders.len() {
            tracing::debug!(
                "reduced {} orders to {} because in flight at last seen block {}, orders in flight: {:?}",
                before_count,
                auction.orders.len(),
                auction.latest_settlement_block,
                inflight_order_uids
            );
        }

        let auction_start_block = auction.block;
        let competition_auction = CompetitionAuction {
            orders: auction
                .orders
                .iter()
                .map(|order| order.metadata.uid)
                .collect(),
            prices: auction.prices.clone(),
        };

        let orders = auction
            .orders
            .into_iter()
            .filter_map(
                |order| match self.order_converter.normalize_limit_order(order) {
                    Ok(order) => Some(order),
                    Err(err) => {
                        // This should never happen unless we are getting malformed
                        // orders from the API - so raise an alert if this happens.
                        tracing::error!(?err, "error normalizing limit order");
                        None
                    }
                },
            )
            .collect::<Vec<_>>();
        tracing::info!(?orders, "got {} orders", orders.len());

        let external_prices =
            ExternalPrices::try_from_auction_prices(self.native_token, auction.prices)
                .context("malformed acution prices")?;
        tracing::debug!(?external_prices, "estimated prices");

        let liquidity = self
            .liquidity_collector
            .get_liquidity_for_orders(&orders, Block::Number(current_block_during_liquidity_fetch))
            .await?;

        self.metrics.orders_fetched(&orders);
        self.metrics.liquidity_fetched(&liquidity);

        if !auction_preprocessing::has_at_least_one_user_order(&orders) {
            return Ok(());
        }

        let gas_price = self
            .gas_price_estimator
            .estimate()
            .await
            .context("failed to estimate gas price")?;
        tracing::debug!("solving with gas price of {:?}", gas_price);

        let next_solver_competition = auction.next_solver_competition;
        let auction = Auction {
            id: auction.next_solver_competition,
            run: run_id,
            orders: orders.clone(),
            liquidity,
            gas_price: gas_price.effective_gas_price(),
            deadline: Instant::now() + self.solver_time_limit,
            external_prices: external_prices.clone(),
        };

        tracing::debug!(deadline =? auction.deadline, "solving auction");
        let run_solver_results = self.run_solvers(auction).await;
        let (mut rated_settlements, errors) = self
            .settlement_ranker
            .rank_legal_settlements(run_solver_results, &external_prices, gas_price)
            .await?;

        // We don't know the exact block because simulation can happen over multiple blocks but
        // this is a good approximation.
        let block_during_simulation = self
            .block_stream
            .borrow()
            .number
            .unwrap_or_default()
            .as_u64();

        DriverLogger::print_settlements(&rated_settlements, &self.fee_objective_scaling_factor);

        // Report solver competition data to the api.
        let mut solver_competition = SolverCompetition {
            gas_price: gas_price.effective_gas_price(),
            auction_start_block,
            liquidity_collected_block: current_block_during_liquidity_fetch,
            competition_simulation_block: block_during_simulation,
            transaction_hash: None,
            auction: competition_auction,
            solutions: rated_settlements
                .iter()
                .map(|(solver, rated_settlement, _)| SolverSettlement {
                    solver: solver.name().to_string(),
                    objective: Objective {
                        total: rated_settlement
                            .objective_value()
                            .to_f64()
                            .unwrap_or(f64::NAN),
                        surplus: rated_settlement.surplus.to_f64().unwrap_or(f64::NAN),
                        fees: rated_settlement
                            .unscaled_subsidized_fee
                            .to_f64()
                            .unwrap_or(f64::NAN),
                        cost: rated_settlement.gas_estimate.to_f64_lossy()
                            * rated_settlement.gas_price.to_f64().unwrap_or(f64::NAN),
                        gas: rated_settlement.gas_estimate.low_u64(),
                    },
                    clearing_prices: rated_settlement
                        .settlement
                        .clearing_prices()
                        .iter()
                        .map(|(address, price)| (*address, *price))
                        .collect(),
                    orders: rated_settlement
                        .settlement
                        .executed_trades()
                        .map(|(trade, _)| solver_competition::Order {
                            id: trade.order.metadata.uid,
                            executed_amount: trade.executed_amount,
                        })
                        .collect(),
                    call_data: settlement_simulation::call_data(
                        rated_settlement.settlement.clone().into(),
                    ),
                })
                .collect(),
        };

        if let Some((winning_solver, mut winning_settlement, _)) = rated_settlements.pop() {
            winning_settlement.settlement = self
                .post_processing_pipeline
                .optimize_settlement(
                    winning_settlement.settlement,
                    winning_solver.account().clone(),
                    gas_price,
                )
                .await;

            tracing::info!(
                "winning settlement id {} by solver {}: {:?}",
                winning_settlement.id,
                winning_solver.name(),
                winning_settlement
            );

            self.metrics
                .complete_runloop_until_transaction(start.elapsed());
            match submit_settlement(
                &self.solution_submitter,
                &self.logger,
                winning_solver.clone(),
                winning_settlement.clone(),
            )
            .await
            {
                Ok(receipt) => {
                    self.update_in_flight_orders(&receipt, &winning_settlement.settlement);
                    solver_competition.transaction_hash = Some(receipt.transaction_hash);
                }
                Err(SubmissionError::Revert(hash)) => {
                    solver_competition.transaction_hash = Some(hash);
                }
                _ => (),
            }

            self.logger.report_on_batch(
                &(winning_solver, winning_settlement),
                rated_settlements
                    .into_iter()
                    .map(|(solver, settlement, _)| (solver, settlement))
                    .collect(),
            );
            self.send_solver_competition(next_solver_competition, solver_competition)
                .await;
        }
        // Happens after settlement submission so that we do not delay it.
        self.logger.report_simulation_errors(
            errors,
            current_block_during_liquidity_fetch,
            gas_price,
        );
        Ok(())
    }

    /// Marks all orders in the winning settlement as "in flight".
    fn update_in_flight_orders(&mut self, receipt: &TransactionReceipt, settlement: &Settlement) {
        let block = match receipt.block_number {
            Some(block) => block.as_u64(),
            None => {
                tracing::error!("tx receipt does not contain block number");
                0
            }
        };
        self.in_flight_orders.mark_settled_orders(block, settlement);
    }

    fn next_run_id(&mut self) -> u64 {
        let id = self.run_id;
        self.run_id += 1;
        id
    }

    async fn send_solver_competition(
        &self,
        expected_id: SolverCompetitionId,
        body: SolverCompetition,
    ) {
        match self.api.send_solver_competition(&body).await {
            Ok(id) if id == expected_id => tracing::info!("stored solver competition"),
            Ok(actual_id) => {
                tracing::warn!(
                    %expected_id, %actual_id,
                    "stored solver competition with unexpected ID",
                );
            }
            Err(err) => tracing::warn!(?err, "failed to send solver competition"),
        }
    }
}

/// Submits the winning solution and handles the related logging and metrics.
pub async fn submit_settlement(
    solution_submitter: &SolutionSubmitter,
    logger: &DriverLogger,
    solver: Arc<dyn Solver>,
    rated_settlement: RatedSettlement,
) -> Result<TransactionReceipt, SubmissionError> {
    let start = Instant::now();
    let result = solution_submitter
        .settle(
            rated_settlement.settlement.clone(),
            rated_settlement.gas_estimate,
            solver.account().clone(),
        )
        .await;
    logger.metrics.transaction_submission(start.elapsed());
    logger
        .log_submission_info(&result, &rated_settlement, &solver)
        .await;
    result
}
