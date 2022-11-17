pub mod solver_settlements;

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
    settlement_simulation,
    settlement_submission::{SolutionSubmitter, SubmissionError},
    solver::{Auction, Solver, Solvers},
};
use anyhow::{anyhow, Context, Result};
use contracts::GPv2Settlement;
use ethcontract::Account;
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use model::{
    auction::AuctionWithId,
    order::OrderUid,
    solver_competition::{
        self, CompetitionAuction, Objective, SolverCompetitionDB, SolverSettlement,
    },
};
use num::{rational::Ratio, BigInt, BigRational, ToPrimitive};
use primitive_types::{H160, U256};
use shared::{
    current_block::{self, CurrentBlockStream},
    ethrpc::Web3,
    http_solver::model::SolverRunError,
    recent_block_cache::Block,
    tenderly_api::TenderlyApi,
    token_list::AutoUpdatingTokenList,
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
    web3: Web3,
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
        market_makable_token_list: AutoUpdatingTokenList,
        block_stream: CurrentBlockStream,
        solution_submitter: SolutionSubmitter,
        api: OrderBookApi,
        order_converter: Arc<OrderConverter>,
        weth_unwrap_factor: f64,
        simulation_gas_limit: u128,
        fee_objective_scaling_factor: f64,
        max_settlement_price_deviation: Option<Ratio<BigInt>>,
        token_list_restriction_for_price_checks: PriceCheckTokens,
        tenderly: Option<Arc<dyn TenderlyApi>>,
        solution_comparison_decimal_cutoff: u16,
    ) -> Self {
        let post_processing_pipeline = PostProcessingPipeline::new(
            native_token,
            web3.clone(),
            weth_unwrap_factor,
            settlement_contract.clone(),
            market_makable_token_list,
        );

        let settlement_rater = Arc::new(SettlementRater {
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
            decimal_cutoff: solution_comparison_decimal_cutoff,
        };

        let logger = DriverLogger {
            metrics: metrics.clone(),
            web3: web3.clone(),
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
            web3,
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
                        Ok(inner) => inner.map_err(Into::into),
                        Err(_timeout) => Err(SolverRunError::Timeout),
                    };
                let response = match &result {
                    Err(SolverRunError::Timeout) => "timeout",
                    Err(_) => "error",
                    Ok(solutions) if solutions.is_empty() => "none",
                    Ok(_) => "solution",
                };
                metrics.settlement_computed(solver.name(), response, start_time);
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

        let id = auction.id;
        let run = self.next_run_id();

        // extra function so that we can add span information
        self.single_auction(auction, run)
            .instrument(tracing::info_span!("auction", id, run))
            .await?;
        Ok(())
    }

    /// Returns whether a settlement was attempted and thus a solver competition info stored.
    async fn single_auction(&mut self, auction: AuctionWithId, run_id: u64) -> Result<()> {
        let start = Instant::now();
        tracing::debug!("starting single run");

        let auction_id = auction.id;
        let mut auction = auction.auction;

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
            .filter_map(|order| {
                let uid = order.metadata.uid;
                match self.order_converter.normalize_limit_order(order) {
                    Ok(mut order) => {
                        order.reward = auction.rewards.get(&uid).copied().unwrap_or(0.);
                        Some(order)
                    }
                    Err(err) => {
                        // This should never happen unless we are getting malformed
                        // orders from the API - so raise an alert if this happens.
                        tracing::error!(?err, "error normalizing limit order");
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        tracing::info!(?orders, "got {} orders", orders.len());

        let external_prices =
            ExternalPrices::try_from_auction_prices(self.native_token, auction.prices)
                .context("malformed auction prices")?;
        tracing::debug!(?external_prices, "estimated prices");

        let liquidity = self
            .liquidity_collector
            .get_liquidity_for_orders(&orders, Block::Number(current_block_during_liquidity_fetch))
            .await?;

        self.metrics.orders_fetched(&orders);
        self.metrics.liquidity_fetched(&liquidity);

        if !auction_preprocessing::has_at_least_one_user_order(&orders)
            || !auction_preprocessing::has_at_least_one_mature_order(&orders)
        {
            return Ok(());
        }

        let gas_price = self
            .gas_price_estimator
            .estimate()
            .await
            .context("failed to estimate gas price")?;
        tracing::debug!("solving with gas price of {:?}", gas_price);

        let rewards = auction.rewards;
        let auction = Auction {
            id: auction_id,
            run: run_id,
            orders: orders.clone(),
            liquidity,
            liquidity_fetch_block: current_block_during_liquidity_fetch,
            gas_price: gas_price.effective_gas_price(),
            deadline: Instant::now() + self.solver_time_limit,
            external_prices: external_prices.clone(),
        };

        tracing::debug!(deadline =? auction.deadline, "solving auction");
        let run_solver_results = self.run_solvers(auction).await;
        let (mut rated_settlements, errors) = self
            .settlement_ranker
            .rank_legal_settlements(run_solver_results, &external_prices, gas_price, auction_id)
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
        let solver_competition = SolverCompetitionDB {
            gas_price: gas_price.effective_gas_price(),
            auction_start_block,
            liquidity_collected_block: current_block_during_liquidity_fetch,
            competition_simulation_block: block_during_simulation,
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
                            .scaled_unsubsidized_fee
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
                        .trades()
                        .map(|trade| solver_competition::Order {
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

            let rewards: Vec<(OrderUid, f64)> = winning_settlement
                .settlement
                .user_trades()
                .map(|trade| {
                    let uid = &trade.order.metadata.uid;
                    let reward = rewards.get(uid).copied().unwrap_or(0.);
                    // Log in case something goes wrong with storing the rewards in the database.
                    tracing::debug!(%uid, %reward, "winning solution reward");
                    (*uid, reward)
                })
                .collect();

            let account = winning_solver.account();
            let address = account.address();
            let nonce = self
                .web3
                .eth()
                .transaction_count(address, None)
                .await
                .context("transaction_count")?;
            let transaction = model::solver_competition::Transaction {
                account: address,
                nonce: nonce
                    .try_into()
                    .map_err(|err| anyhow!("{err}"))
                    .context("convert nonce")?,
            };
            tracing::debug!(?transaction, "winning solution transaction");

            let solver_competition = model::solver_competition::Request {
                auction: auction_id,
                transaction,
                competition: solver_competition,
                rewards,
            };
            // This has to succeed in order to continue settling. Otherwise we can't be sure the
            // competition info has been stored.
            self.send_solver_competition(&solver_competition).await?;

            self.metrics
                .complete_runloop_until_transaction(start.elapsed());
            tracing::debug!(?address, ?nonce, "submitting settlement");
            let hash = match submit_settlement(
                &self.solution_submitter,
                &self.logger,
                account.clone(),
                nonce,
                winning_solver.name(),
                winning_settlement.settlement.clone(),
                winning_settlement.gas_estimate,
                Some(winning_settlement.id as u64),
            )
            .await
            {
                Ok(receipt) => {
                    self.update_in_flight_orders(&receipt, &winning_settlement.settlement);
                    Some(receipt.transaction_hash)
                }
                Err(SubmissionError::Revert(hash)) => Some(hash),
                _ => None,
            };
            if let Some(hash) = hash {
                tracing::debug!(?hash, "settled transaction");
            }

            self.logger.report_on_batch(
                &(winning_solver, winning_settlement),
                rated_settlements
                    .into_iter()
                    .map(|(solver, settlement, _)| (solver, settlement))
                    .collect(),
            );
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
        body: &model::solver_competition::Request,
    ) -> Result<()> {
        // For example shadow solver shouldn't store competition info.
        if !self.api.is_authenticated() {
            return Ok(());
        }
        self.api
            .send_solver_competition(body)
            .await
            .context("send_solver_competition")
    }
}

/// Submits the winning solution and handles the related logging and metrics.
#[allow(clippy::too_many_arguments)]
pub async fn submit_settlement(
    solution_submitter: &SolutionSubmitter,
    logger: &DriverLogger,
    account: Account,
    nonce: U256,
    solver_name: &str,
    settlement: Settlement,
    gas_estimate: U256,
    settlement_id: Option<u64>,
) -> Result<TransactionReceipt, SubmissionError> {
    let start = Instant::now();
    let result = solution_submitter
        .settle(settlement.clone(), gas_estimate, account, nonce)
        .await;
    logger.metrics.transaction_submission(start.elapsed());
    logger
        .log_submission_info(&result, &settlement, settlement_id, solver_name)
        .await;
    result
}
