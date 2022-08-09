pub mod solver_settlements;

use self::solver_settlements::RatedSettlement;
use crate::{
    analytics, auction_preprocessing,
    in_flight_orders::InFlightOrders,
    liquidity::order_converter::OrderConverter,
    liquidity_collector::LiquidityCollector,
    metrics::{SolverMetrics, SolverRunOutcome},
    orderbook::OrderBookApi,
    settlement::{external_prices::ExternalPrices, PriceCheckTokens, Settlement},
    settlement_post_processing::PostProcessingPipeline,
    settlement_rater::{SettlementRater, SettlementRating},
    settlement_simulation::{self, simulate_before_after_access_list, TenderlyApi},
    settlement_submission::{SolutionSubmitter, SubmissionError},
    solver::{Auction, SettlementWithError, Solver, Solvers},
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use futures::future::join_all;
use gas_estimation::{GasPrice1559, GasPriceEstimating};
use itertools::Itertools;
use model::solver_competition::{
    self, Objective, SolverCompetition, SolverCompetitionId, SolverSettlement,
};
use model::{
    order::{Order, OrderKind},
    solver_competition::CompetitionAuction,
};
use num::{rational::Ratio, BigInt, BigRational, ToPrimitive};
use primitive_types::{H160, H256};
use rand::prelude::SliceRandom;
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
use tracing::{Instrument as _, Span};
use web3::types::{AccessList, TransactionReceipt};

pub struct Driver {
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
    max_merged_settlements: usize,
    solver_time_limit: Duration,
    market_makable_token_list: Option<TokenList>,
    block_stream: CurrentBlockStream,
    solution_submitter: SolutionSubmitter,
    run_id: u64,
    max_settlements_per_solver: usize,
    api: OrderBookApi,
    order_converter: Arc<OrderConverter>,
    in_flight_orders: InFlightOrders,
    post_processing_pipeline: PostProcessingPipeline,
    simulation_gas_limit: u128,
    fee_objective_scaling_factor: BigRational,
    max_settlement_price_deviation: Option<Ratio<BigInt>>,
    token_list_restriction_for_price_checks: PriceCheckTokens,
    tenderly: Option<TenderlyApi>,
    settlement_rater: Box<dyn SettlementRating>,
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
        max_merged_settlements: usize,
        solver_time_limit: Duration,
        market_makable_token_list: Option<TokenList>,
        block_stream: CurrentBlockStream,
        solution_submitter: SolutionSubmitter,
        max_settlements_per_solver: usize,
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
        );

        let settlement_rater = Box::new(SettlementRater {
            access_list_estimator: solution_submitter.access_list_estimator.clone(),
            settlement_contract: settlement_contract.clone(),
            web3: web3.clone(),
        });

        Self {
            settlement_contract,
            liquidity_collector,
            solvers,
            gas_price_estimator,
            settle_interval,
            native_token,
            min_order_age,
            metrics,
            web3,
            network_id,
            max_merged_settlements,
            solver_time_limit,
            market_makable_token_list,
            block_stream,
            solution_submitter,
            run_id: 0,
            max_settlements_per_solver,
            api,
            order_converter,
            in_flight_orders: InFlightOrders::default(),
            post_processing_pipeline,
            simulation_gas_limit,
            fee_objective_scaling_factor: BigRational::from_float(fee_objective_scaling_factor)
                .unwrap(),
            max_settlement_price_deviation,
            token_list_restriction_for_price_checks,
            tenderly,
            settlement_rater,
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

    /// Collects all orders which got traded in the settlement. Tapping into partially fillable
    /// orders multiple times will not result in duplicates. Partially fillable orders get
    /// considered as traded only the first time we tap into their liquidity.
    fn get_traded_orders(settlement: &Settlement) -> Vec<Order> {
        let mut traded_orders = Vec::new();
        for (_, group) in &settlement
            .executed_trades()
            .map(|(trade, _)| trade)
            .group_by(|trade| trade.order.metadata.uid)
        {
            let mut group = group.into_iter().peekable();
            let order = &group.peek().unwrap().order;
            let was_already_filled = match order.data.kind {
                OrderKind::Buy => &order.metadata.executed_buy_amount,
                OrderKind::Sell => &order.metadata.executed_sell_amount,
            } > &0u8.into();
            let is_getting_filled = group.any(|trade| !trade.executed_amount.is_zero());
            if !was_already_filled && is_getting_filled {
                traded_orders.push(order.clone());
            }
        }
        traded_orders
    }

    async fn submit_settlement(
        &self,
        solver: Arc<dyn Solver>,
        rated_settlement: RatedSettlement,
    ) -> Result<TransactionReceipt, SubmissionError> {
        let settlement = rated_settlement.settlement;
        let traded_orders = Self::get_traded_orders(&settlement);

        self.metrics
            .settlement_revertable_status(settlement.revertable(), solver.name());

        match self
            .solution_submitter
            .settle(
                settlement,
                rated_settlement.gas_estimate,
                solver.account().clone(),
            )
            .await
        {
            Ok(receipt) => {
                let name = solver.name();
                tracing::info!(
                    settlement_id =% rated_settlement.id,
                    transaction_hash =? receipt.transaction_hash,
                    "Successfully submitted settlement",
                );
                traded_orders
                    .iter()
                    .for_each(|order| self.metrics.order_settled(order, name));
                self.metrics.settlement_submitted(
                    crate::metrics::SettlementSubmissionOutcome::Success,
                    name,
                );
                if let Err(err) = self
                    .metric_access_list_gas_saved(receipt.transaction_hash)
                    .await
                {
                    tracing::debug!(?err, "access list metric not saved");
                }
                Ok(receipt)
            }
            Err(err) => {
                // Since we simulate and only submit solutions when they used to pass before, there is no
                // point in logging transaction failures in the form of race conditions as hard errors.
                tracing::warn!(
                    settlement_id =% rated_settlement.id, ?err,
                    "Failed to submit settlement",
                );
                self.metrics
                    .settlement_submitted(err.as_outcome(), solver.name());
                if let Some(transaction_hash) = err.transaction_hash() {
                    if let Err(err) = self.metric_access_list_gas_saved(transaction_hash).await {
                        tracing::debug!(?err, "access list metric not saved");
                    }
                }
                Err(err)
            }
        }
    }

    async fn metric_access_list_gas_saved(&self, transaction_hash: H256) -> Result<()> {
        let gas_saved = simulate_before_after_access_list(
            &self.web3,
            self.tenderly.as_ref().context("tenderly disabled")?,
            self.network_id.clone(),
            transaction_hash,
        )
        .await?;
        tracing::debug!(?gas_saved, "access list gas saved");
        if gas_saved.is_sign_positive() {
            self.metrics
                .settlement_access_list_saved_gas(gas_saved, "positive");
        } else {
            self.metrics
                .settlement_access_list_saved_gas(-gas_saved, "negative");
        }

        Ok(())
    }

    async fn can_settle_without_liquidity(
        &self,
        solver: Arc<dyn Solver>,
        settlement: &RatedSettlement,
        gas_price: GasPrice1559,
        access_list: Option<AccessList>,
    ) -> Result<bool> {
        // We don't want to buy tokens that we don't trust. If no list is set, we settle with external liquidity.
        if !self
            .market_makable_token_list
            .as_ref()
            .map(|list| is_only_selling_trusted_tokens(&settlement.settlement, list))
            .unwrap_or(false)
        {
            return Ok(false);
        }

        let simulations = settlement_simulation::simulate_and_estimate_gas_at_current_block(
            std::iter::once((
                solver.account().clone(),
                settlement.settlement.without_onchain_liquidity(),
                access_list,
            )),
            &self.settlement_contract,
            &self.web3,
            gas_price,
        )
        .await
        .context("failed to simulate settlement")?;
        Ok(simulations[0].is_ok())
    }

    // Log simulation errors only if the simulation also fails in the block at which on chain
    // liquidity was queried. If the simulation succeeds at the previous block then the solver
    // worked correctly and the error doesn't have to be reported.
    // Note that we could still report a false positive because the earlier block might be off by if
    // the block has changed just as were were querying the node.
    fn report_simulation_errors(
        &self,
        errors: Vec<SettlementWithError>,
        current_block_during_liquidity_fetch: u64,
        gas_price: GasPrice1559,
    ) {
        let contract = self.settlement_contract.clone();
        let web3 = self.web3.clone();
        let network_id = self.network_id.clone();
        let metrics = self.metrics.clone();
        let simulation_gas_limit = self.simulation_gas_limit;
        let task = async move {
            let simulations = settlement_simulation::simulate_and_error_with_tenderly_link(
                errors.iter().map(|(solver, settlement, access_list, _)| {
                    (
                        solver.account().clone(),
                        settlement.clone(),
                        access_list.clone(),
                    )
                }),
                &contract,
                &web3,
                gas_price,
                &network_id,
                current_block_during_liquidity_fetch,
                simulation_gas_limit,
            )
            .await;

            for ((solver, settlement, _, _), result) in errors.iter().zip(simulations) {
                metrics.settlement_simulation_failed_on_latest(solver.name());
                if let Err(error_at_earlier_block) = result {
                    tracing::warn!(
                        "{} settlement simulation failed at submission and block {}:\n{:?}",
                        solver.name(),
                        current_block_during_liquidity_fetch,
                        error_at_earlier_block,
                    );
                    // split warning into separate logs so that the messages aren't too long.
                    tracing::warn!(
                        "{} settlement failure for: \n{:#?}",
                        solver.name(),
                        settlement,
                    );

                    metrics.settlement_simulation_failed(solver.name());
                }
            }
        };
        tokio::task::spawn(task.instrument(Span::current()));
    }

    /// Record metrics on the matched orders from a single batch. Specifically we report on
    /// the number of orders that were;
    ///  - surplus in winning settlement vs unrealized surplus from other feasible solutions.
    ///  - matched but not settled in this runloop (effectively queued for the next one)
    /// Should help us to identify how much we can save by parallelizing execution.
    fn report_on_batch(
        &self,
        submitted: &(Arc<dyn Solver>, RatedSettlement),
        other_settlements: Vec<(Arc<dyn Solver>, RatedSettlement)>,
    ) {
        // Report surplus
        analytics::report_alternative_settlement_surplus(
            &*self.metrics,
            submitted,
            &other_settlements,
        );
        // Report matched but not settled
        analytics::report_matched_but_not_settled(&*self.metrics, submitted, &other_settlements);
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
        self.in_flight_orders.update_and_filter(&mut auction);
        if before_count != auction.orders.len() {
            tracing::debug!(
                "reduced {} orders to {} because in flight at last seen block {}",
                before_count,
                auction.orders.len(),
                auction.block
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

        let mut solver_settlements = Vec::new();

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
        for (solver, settlements) in run_solver_results {
            let name = solver.name();

            let mut settlements = match settlements {
                Ok(mut settlement) => {
                    for settlement in &settlement {
                        tracing::debug!(solver_name = %name, ?settlement, "found solution");
                    }

                    // Do not continue with settlements that are empty or only liquidity orders.
                    let settlement_count = settlement.len();
                    settlement.retain(solver_settlements::has_user_order);
                    if settlement_count != settlement.len() {
                        tracing::debug!(
                            solver_name = %name,
                            "settlement(s) filtered containing only liquidity orders",
                        );
                    }

                    if let Some(max_settlement_price_deviation) =
                        &self.max_settlement_price_deviation
                    {
                        let settlement_count = settlement.len();
                        settlement.retain(|settlement| {
                            settlement.satisfies_price_checks(
                                solver.name(),
                                &external_prices,
                                max_settlement_price_deviation,
                                &self.token_list_restriction_for_price_checks,
                            )
                        });
                        if settlement_count != settlement.len() {
                            tracing::debug!(
                                solver_name = %name,
                                "settlement(s) filtered for violating maximum external price deviation",
                            );
                        }
                    }

                    if settlement.is_empty() {
                        self.metrics.solver_run(SolverRunOutcome::Empty, name);
                        continue;
                    }

                    self.metrics.solver_run(SolverRunOutcome::Success, name);
                    settlement
                }
                Err(err) => {
                    match err {
                        SolverRunError::Timeout => {
                            self.metrics.solver_run(SolverRunOutcome::Timeout, name)
                        }
                        SolverRunError::Solving(_) => {
                            self.metrics.solver_run(SolverRunOutcome::Failure, name)
                        }
                    }
                    tracing::warn!(solver_name = %name, ?err, "solver error");
                    continue;
                }
            };

            // Keep at most this many settlements. This is important in case where a solver produces
            // a large number of settlements which would hold up the driver logic when simulating
            // them.
            // Shuffle first so that in the case a buggy solver keeps returning some amount of
            // invalid settlements first we have a chance to make progress.
            settlements.shuffle(&mut rand::thread_rng());
            settlements.truncate(self.max_settlements_per_solver);

            solver_settlements::merge_settlements(
                self.max_merged_settlements,
                &external_prices,
                &mut settlements,
            );

            solver_settlements.reserve(settlements.len());

            for settlement in settlements {
                solver_settlements.push((solver.clone(), settlement))
            }
        }

        // filters out all non-mature settlements
        let solver_settlements =
            solver_settlements::retain_mature_settlements(self.min_order_age, solver_settlements);

        // log considered settlements. While we already log all found settlements, this additonal
        // statement allows us to figure out which settlements were filtered out and which ones are
        // going to be simulated and considered for competition.
        for (solver, settlement) in &solver_settlements {
            tracing::debug!(
                solver_name = %solver.name(), ?settlement,
                "considering solution for solver competition",
            );
        }

        let (mut rated_settlements, errors) = self
            .settlement_rater
            .rate_settlements(solver_settlements, &external_prices, gas_price)
            .await?;

        // We don't know the exact block because simulation can happen over multiple blocks but
        // this is a good approximation.
        let block_during_simulation = self
            .block_stream
            .borrow()
            .number
            .unwrap_or_default()
            .as_u64();
        tracing::info!(
            "{} settlements passed simulation and {} failed",
            rated_settlements.len(),
            errors.len(),
        );
        for (solver, _, _) in &rated_settlements {
            self.metrics.settlement_simulation_succeeded(solver.name());
        }

        // Before sorting, make sure to shuffle the settlements. This is to make sure we don't give
        // preference to any specific solver when there is an objective value tie.
        rated_settlements.shuffle(&mut rand::thread_rng());

        rated_settlements.sort_by(|a, b| a.1.objective_value().cmp(&b.1.objective_value()));
        print_settlements(&rated_settlements, &self.fee_objective_scaling_factor);

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

        if let Some((winning_solver, mut winning_settlement, access_list)) = rated_settlements.pop()
        {
            // If we have enough buffer in the settlement contract to not use on-chain interactions, remove those
            if self
                .can_settle_without_liquidity(
                    winning_solver.clone(),
                    &winning_settlement,
                    gas_price,
                    access_list.clone(),
                )
                .await
                .unwrap_or(false)
            {
                winning_settlement.settlement =
                    winning_settlement.settlement.without_onchain_liquidity();
                tracing::debug!("settlement without onchain liquidity");
            }

            tracing::info!(
                "winning settlement id {} by solver {}: {:?}",
                winning_settlement.id,
                winning_solver.name(),
                winning_settlement
            );

            winning_settlement.settlement = self
                .post_processing_pipeline
                .optimize_settlement(
                    winning_settlement.settlement,
                    access_list,
                    winning_solver.account().clone(),
                    gas_price,
                )
                .await;

            self.metrics
                .complete_runloop_until_transaction(start.elapsed());
            let start = Instant::now();
            match self
                .submit_settlement(winning_solver.clone(), winning_settlement.clone())
                .await
            {
                Ok(receipt) => {
                    let block = match receipt.block_number {
                        Some(block) => block.as_u64(),
                        None => {
                            tracing::error!("tx receipt does not contain block number");
                            0
                        }
                    };

                    self.in_flight_orders
                        .mark_settled_orders(block, &winning_settlement.settlement);

                    match receipt.effective_gas_price {
                        Some(price) => {
                            self.metrics.transaction_gas_price(price);
                        }
                        None => {
                            tracing::error!(
                                "node did not return effective gas price in tx receipt"
                            );
                        }
                    }

                    solver_competition.transaction_hash = Some(receipt.transaction_hash);
                }
                Err(SubmissionError::Revert(hash)) => {
                    solver_competition.transaction_hash = Some(hash);
                }
                _ => (),
            }

            self.metrics.transaction_submission(start.elapsed());
            self.report_on_batch(
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
        self.report_simulation_errors(errors, current_block_during_liquidity_fetch, gas_price);
        Ok(())
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

fn is_only_selling_trusted_tokens(settlement: &Settlement, token_list: &TokenList) -> bool {
    !settlement
        .traded_orders()
        .any(|order| token_list.get(&order.data.sell_token).is_none())
}

fn print_settlements(
    rated_settlements: &[(Arc<dyn Solver>, RatedSettlement, Option<AccessList>)],
    fee_objective_scaling_factor: &BigRational,
) {
    let mut text = String::new();
    for (solver, settlement, access_list) in rated_settlements {
        use std::fmt::Write;
        write!(
            text,
            "\nid={} solver={} \
             objective={:.2e} surplus={:.2e} \
             gas_estimate={:.2e} gas_price={:.2e} \
             unscaled_unsubsidized_fee={:.2e} unscaled_subsidized_fee={:.2e} \
             access_list_addreses={}",
            settlement.id,
            solver.name(),
            settlement.objective_value().to_f64().unwrap_or(f64::NAN),
            settlement.surplus.to_f64().unwrap_or(f64::NAN),
            settlement.gas_estimate.to_f64_lossy(),
            settlement.gas_price.to_f64().unwrap_or(f64::NAN),
            (&settlement.scaled_unsubsidized_fee / fee_objective_scaling_factor)
                .to_f64()
                .unwrap_or(f64::NAN),
            settlement
                .unscaled_subsidized_fee
                .to_f64()
                .unwrap_or(f64::NAN),
            access_list.clone().unwrap_or_default().len()
        )
        .unwrap();
    }
    tracing::info!("Rated Settlements: {}", text);
}

#[derive(Debug)]
enum SolverRunError {
    Timeout,
    Solving(anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        settlement::{OrderTrade, Trade},
        solver::dummy_arc_solver,
    };
    use maplit::hashmap;
    use model::order::{Order, OrderData};
    use shared::token_list::Token;
    use std::collections::HashMap;

    #[test]
    fn test_is_only_selling_trusted_tokens() {
        let good_token = H160::from_low_u64_be(1);
        let another_good_token = H160::from_low_u64_be(2);
        let bad_token = H160::from_low_u64_be(3);

        let token_list = TokenList::new(hashmap! {
            good_token => Token {
                address: good_token,
                symbol: "Foo".into(),
                name: "FooCoin".into(),
                decimals: 18,
            },
            another_good_token => Token {
                address: another_good_token,
                symbol: "Bar".into(),
                name: "BarCoin".into(),
                decimals: 18,
            }
        });

        let trade = |token| OrderTrade {
            trade: Trade {
                order: Order {
                    data: OrderData {
                        sell_token: token,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let settlement = Settlement::with_trades(
            HashMap::new(),
            vec![trade(good_token), trade(another_good_token)],
            vec![],
        );
        assert!(is_only_selling_trusted_tokens(&settlement, &token_list));

        let settlement = Settlement::with_trades(
            HashMap::new(),
            vec![
                trade(good_token),
                trade(another_good_token),
                trade(bad_token),
            ],
            vec![],
        );
        assert!(!is_only_selling_trusted_tokens(&settlement, &token_list));
    }

    #[test]
    #[ignore]
    fn print_settlements() {
        let a = [
            (
                dummy_arc_solver(),
                RatedSettlement {
                    id: 0,
                    settlement: Default::default(),
                    surplus: BigRational::new(1u8.into(), 1u8.into()),
                    unscaled_subsidized_fee: BigRational::new(2u8.into(), 1u8.into()),
                    scaled_unsubsidized_fee: BigRational::new(3u8.into(), 1u8.into()),
                    gas_estimate: 4.into(),
                    gas_price: BigRational::new(5u8.into(), 1u8.into()),
                },
                None,
            ),
            (
                dummy_arc_solver(),
                RatedSettlement {
                    id: 6,
                    settlement: Default::default(),
                    surplus: BigRational::new(7u8.into(), 1u8.into()),
                    unscaled_subsidized_fee: BigRational::new(8u8.into(), 1u8.into()),
                    scaled_unsubsidized_fee: BigRational::new(9u8.into(), 1u8.into()),
                    gas_estimate: 10.into(),
                    gas_price: BigRational::new(11u8.into(), 1u8.into()),
                },
                None,
            ),
        ];

        shared::tracing::initialize_for_tests("INFO");
        super::print_settlements(&a, &BigRational::new(1u8.into(), 2u8.into()));
    }
}
