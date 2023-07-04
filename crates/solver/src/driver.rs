use {
    crate::{settlement, settlement_rater::SettlementRating},
    tracing::info_span,
};

pub mod gas;
pub mod solver_settlements;

use {
    crate::{
        auction_preprocessing,
        driver_logger::DriverLogger,
        in_flight_orders::InFlightOrders,
        liquidity_collector::{LiquidityCollecting, LiquidityCollector},
        metrics::SolverMetrics,
        order_balance_filter,
        orderbook::OrderBookApi,
        settlement::{PriceCheckTokens, Settlement},
        settlement_ranker::SettlementRanker,
        settlement_simulation,
        settlement_submission::{SolutionSubmitter, SubmissionError},
        solver::{Auction, Solver, Solvers},
    },
    anyhow::{anyhow, Context, Result},
    contracts::GPv2Settlement,
    ethcontract::Account,
    futures::future::join_all,
    gas_estimation::GasPriceEstimating,
    model::{
        auction::{AuctionId, AuctionWithId},
        order::OrderUid,
        solver_competition::{
            self,
            CompetitionAuction,
            Execution,
            Objective,
            SolverCompetitionDB,
            SolverSettlement,
        },
        TokenPair,
    },
    num::{rational::Ratio, BigInt, ToPrimitive},
    primitive_types::{H160, U256},
    shared::{
        account_balances::BalanceFetching,
        current_block::CurrentBlockStream,
        ethrpc::Web3,
        external_prices::ExternalPrices,
        http_solver::model::{InternalizationStrategy, SolverRunError},
        recent_block_cache::Block,
        tenderly_api::TenderlyApi,
    },
    std::{
        collections::HashSet,
        sync::Arc,
        time::{Duration, Instant},
    },
    tracing::Instrument as _,
    web3::types::TransactionReceipt,
};

pub struct Driver {
    liquidity_collector: LiquidityCollector,
    solvers: Solvers,
    gas_price_estimator: gas::Estimator,
    settle_interval: Duration,
    native_token: H160,
    metrics: Arc<dyn SolverMetrics>,
    solver_time_limit: Duration,
    block_time: Duration,
    additional_mining_deadline: Duration,
    block_stream: CurrentBlockStream,
    solution_submitter: SolutionSubmitter,
    run_id: u64,
    api: OrderBookApi,
    in_flight_orders: InFlightOrders,
    settlement_ranker: SettlementRanker,
    logger: DriverLogger,
    web3: Web3,
    last_attempted_settlement: Option<AuctionId>,
    process_partially_fillable_liquidity_orders: bool,
    process_partially_fillable_limit_orders: bool,
    balance_fetcher: Arc<dyn BalanceFetching>,
    settlement_encoding_contracts: settlement::Contracts,
}
impl Driver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settlement_contract: GPv2Settlement,
        settlement_encoding_contracts: settlement::Contracts,
        liquidity_collector: LiquidityCollector,
        solvers: Solvers,
        gas_price_estimator: Arc<dyn GasPriceEstimating>,
        gas_price_cap: f64,
        settle_interval: Duration,
        native_token: H160,
        metrics: Arc<dyn SolverMetrics>,
        web3: Web3,
        network_id: String,
        solver_time_limit: Duration,
        block_time: Duration,
        additional_mining_deadline: Duration,
        skip_non_positive_score_settlements: bool,
        block_stream: CurrentBlockStream,
        solution_submitter: SolutionSubmitter,
        api: OrderBookApi,
        simulation_gas_limit: u128,
        max_settlement_price_deviation: Option<Ratio<BigInt>>,
        token_list_restriction_for_price_checks: PriceCheckTokens,
        tenderly: Option<Arc<dyn TenderlyApi>>,
        solution_comparison_decimal_cutoff: u16,
        process_partially_fillable_liquidity_orders: bool,
        process_partially_fillable_limit_orders: bool,
        settlement_rater: Arc<dyn SettlementRating>,
        balance_fetcher: Arc<dyn BalanceFetching>,
    ) -> Self {
        let gas_price_estimator =
            gas::Estimator::new(gas_price_estimator).with_gas_price_cap(gas_price_cap);

        let settlement_ranker = SettlementRanker {
            settlement_encoding_contracts: settlement_encoding_contracts.clone(),
            max_settlement_price_deviation,
            token_list_restriction_for_price_checks,
            metrics: metrics.clone(),
            settlement_rater,
            decimal_cutoff: solution_comparison_decimal_cutoff,
            skip_non_positive_score_settlements,
        };

        let logger = DriverLogger {
            metrics: metrics.clone(),
            web3: web3.clone(),
            tenderly,
            network_id,
            settlement_contract,
            settlement_encoding_contracts: settlement_encoding_contracts.clone(),
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
            block_time,
            additional_mining_deadline,
            block_stream,
            solution_submitter,
            run_id: 0,
            api,
            in_flight_orders: InFlightOrders::default(),
            settlement_ranker,
            logger,
            web3,
            last_attempted_settlement: None,
            process_partially_fillable_liquidity_orders,
            process_partially_fillable_limit_orders,
            balance_fetcher,
            settlement_encoding_contracts,
        }
    }

    pub async fn run_forever(&mut self) -> ! {
        loop {
            let start = Instant::now();
            match self.single_run().await {
                Ok(()) => tracing::debug!("single run finished ok"),
                Err(err) => tracing::error!("single run errored: {:?}", err),
            }
            self.metrics.runloop_completed();
            tokio::time::sleep_until((start + self.settle_interval).into()).await;
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
                let span = info_span!("solver", solver = solver.name());
                let result =
                    match tokio::time::timeout_at(auction.deadline.into(), solver.solve(auction))
                        .instrument(span)
                        .await
                    {
                        Ok(inner) => {
                            inner.map_err(|err| SolverRunError::Solving(format!("{err:?}")))
                        }
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

        // It doesn't make sense to solve the same auction again because we wouldn't be
        // able to store competition info etc.
        if self.last_attempted_settlement == Some(auction.id) {
            tracing::debug!("skipping run because auction hasn't changed {}", auction.id);
            return Ok(());
        }

        let id = auction.id;
        let run = self.next_run_id();

        // extra function so that we can add span information
        let settlement_attempted = self
            .single_auction(auction, run)
            .instrument(tracing::info_span!("auction", id, run))
            .await?;

        if settlement_attempted {
            self.last_attempted_settlement = Some(id);
        }

        Ok(())
    }

    /// Returns whether a settlement transaction was attempted.
    async fn single_auction(&mut self, auction: AuctionWithId, run_id: u64) -> Result<bool> {
        let start = Instant::now();
        tracing::debug!("starting single run");

        let auction_id = auction.id;
        let mut auction = auction.auction;

        let current_block_during_liquidity_fetch = self.block_stream.borrow().number;

        let before_count = auction.orders.len();
        let inflight_order_uids = self.in_flight_orders.update_and_filter(&mut auction);
        if before_count != auction.orders.len() {
            tracing::debug!(
                "reduced {} orders to {} because in flight at last seen block {}, orders in \
                 flight: {:?}",
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

        auction.orders.retain(|order| {
            match (
                order.data.partially_fillable,
                order.metadata.is_liquidity_order,
            ) {
                (false, _) => true,
                (true, true) => self.process_partially_fillable_liquidity_orders,
                (true, false) => self.process_partially_fillable_limit_orders,
            }
        });

        let balance_start = Instant::now();
        let balances =
            order_balance_filter::fetch_balances(self.balance_fetcher.as_ref(), &auction.orders)
                .await;
        tracing::debug!(
            "fetching order balances took {}s",
            balance_start.elapsed().as_secs_f32()
        );

        tracing::info!(count =% auction.orders.len(), "got orders");
        self.metrics.orders_fetched(&auction.orders);

        let auction_prices = auction.prices.clone();
        let external_prices =
            ExternalPrices::try_from_auction_prices(self.native_token, auction.prices)
                .context("malformed auction prices")?;
        tracing::debug!(?external_prices, "estimated prices");

        if !auction_preprocessing::has_at_least_one_user_order(&auction.orders) {
            return Ok(false);
        }

        let gas_price = self
            .gas_price_estimator
            .estimate()
            .await
            .context("failed to estimate gas price")?;
        tracing::debug!("solving with gas price of {:?}", gas_price);

        let pairs: HashSet<_> = auction
            .orders
            .iter()
            .filter(|o| !o.metadata.is_liquidity_order)
            .flat_map(|o| TokenPair::new(o.data.buy_token, o.data.sell_token))
            .collect();
        let liquidity = self
            .liquidity_collector
            .get_liquidity(pairs, Block::Number(current_block_during_liquidity_fetch))
            .await?;
        self.metrics.liquidity_fetched(&liquidity);

        let auction = Auction {
            id: auction_id,
            run: run_id,
            orders: auction.orders,
            liquidity,
            liquidity_fetch_block: current_block_during_liquidity_fetch,
            gas_price: gas_price.effective_gas_price(),
            deadline: Instant::now() + self.solver_time_limit,
            external_prices: external_prices.clone(),
            balances,
        };

        tracing::debug!(deadline =? auction.deadline, "solving auction");
        let run_solver_results = self.run_solvers(auction).await;
        let (mut rated_settlements, errors) = self
            .settlement_ranker
            .rank_legal_settlements(run_solver_results, &external_prices, gas_price, auction_id)
            .await?;

        // We don't know the exact block because simulation can happen over multiple
        // blocks but this is a good approximation.
        let block_during_simulation = self.block_stream.borrow().number;

        DriverLogger::print_settlements(&rated_settlements);

        // Report solver competition data to the api.
        let solver_competition = SolverCompetitionDB {
            gas_price: gas_price.effective_gas_price(),
            auction_start_block,
            liquidity_collected_block: current_block_during_liquidity_fetch,
            competition_simulation_block: block_during_simulation,
            auction: competition_auction,
            solutions: rated_settlements
                .iter()
                .map(|(solver, rated_settlement)| SolverSettlement {
                    solver: solver.name().to_string(),
                    solver_address: solver.account().address(),
                    objective: Objective {
                        total: rated_settlement
                            .objective_value
                            .to_f64()
                            .unwrap_or(f64::NAN),
                        surplus: rated_settlement.surplus.to_f64().unwrap_or(f64::NAN),
                        fees: rated_settlement.solver_fees.to_f64().unwrap_or(f64::NAN),
                        cost: rated_settlement.gas_estimate.to_f64_lossy()
                            * rated_settlement.gas_price.to_f64().unwrap_or(f64::NAN),
                        gas: rated_settlement.gas_estimate.low_u64(),
                    },
                    score: Some(rated_settlement.score),
                    ranking: Some(rated_settlement.ranking),
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
                        rated_settlement.settlement.clone().encode(
                            &self.settlement_encoding_contracts,
                            InternalizationStrategy::SkipInternalizableInteraction,
                        ), // rating is done with internalizations
                    ),
                    uninternalized_call_data: rated_settlement
                        .settlement
                        .clone()
                        .encode_uninternalized_if_different(&self.settlement_encoding_contracts)
                        .map(settlement_simulation::call_data),
                })
                .collect(),
        };

        let mut settlement_transaction_attempted = false;
        if let Some((winning_solver, winning_settlement)) = rated_settlements.pop() {
            tracing::info!(
                "winning settlement id {} by solver {}: {:?}",
                winning_settlement.id,
                winning_solver.name(),
                winning_settlement
            );

            let executions: Vec<(OrderUid, Execution)> = winning_settlement
                .settlement
                .user_trades()
                .map(|trade| {
                    let execution = Execution {
                        surplus_fee: trade.surplus_fee(),
                        solver_fee: trade.solver_fee,
                    };
                    (trade.order.metadata.uid, execution)
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
            let scores = model::solver_competition::Scores {
                winner: address,
                winning_score: winning_settlement.score.score(),
                // second highest score, or 0 if there is only one score (see CIP20)
                reference_score: rated_settlements
                    .last()
                    .map(|(_, settlement)| settlement.score.score())
                    .unwrap_or_default(),
                block_deadline: {
                    let deadline = self.solver_time_limit
                        + self.solution_submitter.max_confirm_time
                        + self.additional_mining_deadline;
                    let number_blocks = deadline.as_secs() / self.block_time.as_secs();
                    block_during_simulation + number_blocks
                },
            };
            let participants = solver_competition
                .solutions
                .iter()
                .map(|solution| solution.solver_address)
                .collect(); // to avoid duplicates

            // external prices for all tokens contained in the trades of a settlement
            let prices = winning_settlement
                .settlement
                .trades()
                .flat_map(|trade| {
                    let sell_token = trade.order.data.sell_token;
                    let buy_token = trade.order.data.buy_token;
                    let sell_price = auction_prices.get(&sell_token).cloned().unwrap_or_default();
                    let buy_price = auction_prices.get(&buy_token).cloned().unwrap_or_default();
                    [(sell_token, sell_price), (buy_token, buy_price)]
                })
                .collect();
            tracing::debug!(?transaction, "winning solution transaction");

            let solver_competition = model::solver_competition::Request {
                auction: auction_id,
                transaction,
                competition: solver_competition,
                executions,
                scores,
                participants,
                prices,
            };
            tracing::debug!(?solver_competition, "submitting competition info");

            // This has to succeed in order to continue settling. Otherwise we can't be sure
            // the competition info has been stored.
            self.send_solver_competition(&solver_competition).await?;

            self.metrics
                .complete_runloop_until_transaction(start.elapsed());
            tracing::debug!(?address, ?nonce, "submitting settlement");
            settlement_transaction_attempted = true;
            let hash = match submit_settlement(
                &self.solution_submitter,
                &self.logger,
                account.clone(),
                nonce,
                winning_solver.name(),
                winning_settlement.settlement.clone(),
                winning_settlement.gas_estimate,
                gas_price.max_fee_per_gas,
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
                    .map(|(solver, settlement)| (solver, settlement))
                    .collect(),
            );
        }
        // Happens after settlement submission so that we do not delay it.
        self.logger.report_simulation_errors(
            errors,
            current_block_during_liquidity_fetch,
            gas_price,
        );
        Ok(settlement_transaction_attempted)
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
    max_fee_per_gas: f64,
    settlement_id: Option<u64>,
) -> Result<TransactionReceipt, SubmissionError> {
    let start = Instant::now();
    let result = solution_submitter
        .settle(
            settlement.clone(),
            gas_estimate,
            max_fee_per_gas,
            account,
            nonce,
        )
        .await;
    logger
        .log_submission_info(
            &result,
            &settlement,
            settlement_id,
            solver_name,
            start.elapsed(),
        )
        .await;
    result.map(Into::into)
}
