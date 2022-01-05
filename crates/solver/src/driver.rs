pub mod solver_settlements;

use self::solver_settlements::RatedSettlement;
use crate::{
    analytics, auction_preprocessing,
    in_flight_orders::InFlightOrders,
    liquidity::order_converter::OrderConverter,
    liquidity_collector::LiquidityCollector,
    metrics::{SolverMetrics, SolverRunOutcome},
    orderbook::OrderBookApi,
    settlement::Settlement,
    settlement_post_processing::PostProcessingPipeline,
    settlement_simulation,
    settlement_submission::SolutionSubmitter,
    solver::{Auction, SettlementWithSolver, Solver, Solvers},
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use ethcontract::errors::ExecutionError;
use futures::future::join_all;
use gas_estimation::{EstimatedGasPrice, GasPriceEstimating};
use itertools::{Either, Itertools};
use num::{BigRational, ToPrimitive};
use primitive_types::{H160, U256};
use rand::prelude::SliceRandom;
use shared::{
    current_block::{self, CurrentBlockStream},
    price_estimation::PriceEstimating,
    recent_block_cache::Block,
    token_list::TokenList,
    Web3,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use web3::types::TransactionReceipt;

pub struct Driver {
    settlement_contract: GPv2Settlement,
    liquidity_collector: LiquidityCollector,
    price_estimator: Arc<dyn PriceEstimating>,
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
    solve_id: u64,
    native_token_amount_to_estimate_prices_with: U256,
    max_settlements_per_solver: usize,
    api: OrderBookApi,
    order_converter: OrderConverter,
    in_flight_orders: InFlightOrders,
    post_processing_pipeline: PostProcessingPipeline,
}
impl Driver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settlement_contract: GPv2Settlement,
        liquidity_collector: LiquidityCollector,
        price_estimator: Arc<dyn PriceEstimating>,
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
        native_token_amount_to_estimate_prices_with: U256,
        max_settlements_per_solver: usize,
        api: OrderBookApi,
        order_converter: OrderConverter,
        weth_unwrap_factor: f64,
    ) -> Self {
        let post_processing_pipeline = PostProcessingPipeline::new(
            native_token,
            web3.clone(),
            weth_unwrap_factor,
            settlement_contract.clone(),
        );

        Self {
            settlement_contract,
            liquidity_collector,
            price_estimator,
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
            solve_id: 0,
            native_token_amount_to_estimate_prices_with,
            max_settlements_per_solver,
            api,
            order_converter,
            in_flight_orders: InFlightOrders::default(),
            post_processing_pipeline,
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

    async fn submit_settlement(
        &self,
        solver: Arc<dyn Solver>,
        rated_settlement: RatedSettlement,
    ) -> Result<TransactionReceipt> {
        let settlement = rated_settlement.settlement;
        let trades = settlement.trades().to_vec();
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
                    "Successfully submitted {} settlement: {:?}",
                    name,
                    receipt.transaction_hash
                );
                trades
                    .iter()
                    .for_each(|trade| self.metrics.order_settled(&trade.order, name));
                self.metrics.settlement_submitted(
                    crate::metrics::SettlementSubmissionOutcome::Success,
                    name,
                );
                Ok(receipt)
            }
            Err(err) => {
                // Since we simulate and only submit solutions when they used to pass before, there is no
                // point in logging transaction failures in the form of race conditions as hard errors.
                tracing::warn!("Failed to submit {} settlement: {:?}", solver.name(), err);
                self.metrics
                    .settlement_submitted(err.as_outcome(), solver.name());
                Err(err.into_anyhow())
            }
        }
    }

    async fn can_settle_without_liquidity(
        &self,
        solver: Arc<dyn Solver>,
        settlement: &RatedSettlement,
        gas_price: EstimatedGasPrice,
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
        errors: Vec<(Arc<dyn Solver>, Settlement, ExecutionError)>,
        current_block_during_liquidity_fetch: u64,
        gas_price: EstimatedGasPrice,
    ) {
        let contract = self.settlement_contract.clone();
        let web3 = self.web3.clone();
        let network_id = self.network_id.clone();
        let metrics = self.metrics.clone();
        let task = async move {
            let simulations = settlement_simulation::simulate_and_error_with_tenderly_link(
                errors
                    .iter()
                    .map(|(solver, settlement, _)| (solver.account().clone(), settlement.clone())),
                &contract,
                &web3,
                gas_price,
                &network_id,
                current_block_during_liquidity_fetch,
            )
            .await;

            for ((solver, settlement, _previous_error), result) in errors.iter().zip(simulations) {
                metrics.settlement_simulation_failed_on_latest(solver.name());
                if let Err(error_at_earlier_block) = result {
                    tracing::warn!(
                        "{} settlement simulation failed at submission and block {}:\n{:?}",
                        solver.name(),
                        current_block_during_liquidity_fetch,
                        error_at_earlier_block,
                    );
                    // split warning into separate logs so that the messages aren't too long.
                    tracing::warn!("settlement failure for: \n{:#?}", settlement);

                    metrics.settlement_simulation_failed(solver.name());
                }
            }
        };
        tokio::task::spawn(task);
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

    // Rate settlements, ignoring those for which the rating procedure failed.
    async fn rate_settlements(
        &self,
        settlements: Vec<SettlementWithSolver>,
        prices: &HashMap<H160, BigRational>,
        gas_price: EstimatedGasPrice,
    ) -> Result<(
        Vec<(Arc<dyn Solver>, RatedSettlement)>,
        Vec<(Arc<dyn Solver>, Settlement, ExecutionError)>,
    )> {
        let simulations = settlement_simulation::simulate_and_estimate_gas_at_current_block(
            settlements
                .iter()
                .map(|settlement| (settlement.0.account().clone(), settlement.1.clone())),
            &self.settlement_contract,
            &self.web3,
            gas_price,
        )
        .await
        .context("failed to simulate settlements")?;

        // Normalize gas_price_wei to the native token price in the prices vector.
        let gas_price_normalized = BigRational::from_float(gas_price.effective_gas_price())
            .expect("Invalid gas price.")
            * prices
                .get(&self.native_token)
                .expect("Price of native token must be known.");

        let rate_settlement = |settlement: Settlement, gas_estimate| {
            let surplus = settlement.total_surplus(prices);
            let scaled_solver_fees = settlement.total_scaled_unsubsidized_fees(prices);
            RatedSettlement {
                settlement,
                surplus,
                solver_fees: scaled_solver_fees,
                gas_estimate,
                gas_price: gas_price_normalized.clone(),
            }
        };
        Ok(settlements.into_iter().zip(simulations).partition_map(
            |((solver, settlement), result)| match result {
                Ok(gas_estimate) => {
                    Either::Left((solver.clone(), rate_settlement(settlement, gas_estimate)))
                }
                Err(err) => Either::Right((solver, settlement, err)),
            },
        ))
    }

    pub async fn single_run(&mut self) -> Result<()> {
        let start = Instant::now();
        tracing::debug!("starting single run");

        let current_block_during_liquidity_fetch =
            current_block::block_number(&self.block_stream.borrow())?;

        let orders = self.api.get_orders().await.context("get_orders")?;
        let (before_count, block) = (orders.orders.len(), orders.latest_settlement_block);
        let orders = self.in_flight_orders.update_and_filter(orders);
        if before_count != orders.len() {
            tracing::debug!(
                "reduced {} orders to {} because in flight at last seen block {}",
                before_count,
                orders.len(),
                block
            );
        }
        let orders = orders
            .into_iter()
            .map(|order| self.order_converter.normalize_limit_order(order))
            .collect::<Vec<_>>();
        tracing::info!("got {} orders: {:?}", orders.len(), orders);
        let liquidity = self
            .liquidity_collector
            .get_liquidity_for_orders(&orders, Block::Number(current_block_during_liquidity_fetch))
            .await?;
        let estimated_prices = auction_preprocessing::collect_estimated_prices(
            self.price_estimator.as_ref(),
            self.native_token_amount_to_estimate_prices_with,
            self.native_token,
            &orders,
        )
        .await;
        tracing::debug!("estimated prices: {:?}", estimated_prices);
        let orders = auction_preprocessing::orders_with_price_estimates(orders, &estimated_prices);

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

        let auction = Auction {
            id: self.next_auction_id(),
            orders: orders.clone(),
            liquidity,
            gas_price: gas_price.effective_gas_price(),
            deadline: Instant::now() + self.solver_time_limit,
            price_estimates: estimated_prices.clone(),
        };
        tracing::debug!("solving auction ID {}", auction.id);
        let run_solver_results = self.run_solvers(auction).await;
        for (solver, settlements) in run_solver_results {
            let name = solver.name();

            let mut settlements = match settlements {
                Ok(mut settlement) => {
                    // Do not continue with settlements that are empty or only liquidity orders.
                    settlement.retain(solver_settlements::has_user_order);
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
                    tracing::warn!("solver {} error: {:?}", name, err);
                    continue;
                }
            };

            for settlement in &settlements {
                tracing::debug!("solver {} found solution:\n{:?}", name, settlement);
            }

            // Keep at most this many settlements. This is important in case where a solver produces
            // a large number of settlements which would hold up the driver logic when simulating
            // them.
            // Shuffle first so that in the case a buggy solver keeps returning some amount of
            // invalid settlements first we have a chance to make progress.
            settlements.shuffle(&mut rand::thread_rng());
            settlements.truncate(self.max_settlements_per_solver);

            solver_settlements::merge_settlements(
                self.max_merged_settlements,
                &estimated_prices,
                &mut settlements,
            );

            let mature_settlements =
                solver_settlements::retain_mature_settlements(self.min_order_age, settlements);

            solver_settlements.reserve(mature_settlements.len());
            for settlement in mature_settlements {
                solver_settlements.push((solver.clone(), settlement))
            }
        }

        let (mut rated_settlements, errors) = self
            .rate_settlements(solver_settlements, &estimated_prices, gas_price)
            .await?;
        tracing::info!(
            "{} settlements passed simulation and {} failed",
            rated_settlements.len(),
            errors.len()
        );
        for (solver, _) in &rated_settlements {
            self.metrics.settlement_simulation_succeeded(solver.name());
        }

        rated_settlements.sort_by(|a, b| a.1.objective_value().cmp(&b.1.objective_value()));
        print_settlements(&rated_settlements);
        if let Some((winning_solver, mut winning_settlement)) = rated_settlements.pop() {
            // If we have enough buffer in the settlement contract to not use on-chain interactions, remove those
            if self
                .can_settle_without_liquidity(
                    winning_solver.clone(),
                    &winning_settlement,
                    gas_price,
                )
                .await
                .unwrap_or(false)
            {
                winning_settlement.settlement =
                    winning_settlement.settlement.without_onchain_liquidity();
                tracing::debug!("settlement without onchain liquidity");
            }

            tracing::info!(
                "winning settlement by {}: {:?}",
                winning_solver.name(),
                winning_settlement
            );

            winning_settlement.settlement = self
                .post_processing_pipeline
                .optimize_settlement(
                    winning_settlement.settlement,
                    winning_solver.account().clone(),
                    gas_price,
                )
                .await;

            self.metrics
                .complete_runloop_until_transaction(start.elapsed());
            let start = Instant::now();
            if let Ok(receipt) = self
                .submit_settlement(winning_solver.clone(), winning_settlement.clone())
                .await
            {
                let orders = winning_settlement
                    .settlement
                    .trades()
                    .iter()
                    .map(|t| t.order.order_meta_data.uid);
                let block = match receipt.block_number {
                    Some(block) => block.as_u64(),
                    None => {
                        tracing::error!("tx receipt does not contain block number");
                        0
                    }
                };
                self.in_flight_orders.mark_settled_orders(block, orders);
                self.metrics
                    .transaction_gas_price(receipt.effective_gas_price);
            }
            self.metrics.transaction_submission(start.elapsed());

            self.report_on_batch(&(winning_solver, winning_settlement), rated_settlements);
        }
        // Happens after settlement submission so that we do not delay it.
        self.report_simulation_errors(errors, current_block_during_liquidity_fetch, gas_price);
        Ok(())
    }

    fn next_auction_id(&mut self) -> u64 {
        let id = self.solve_id;
        self.solve_id += 1;
        id
    }
}

fn is_only_selling_trusted_tokens(settlement: &Settlement, token_list: &TokenList) -> bool {
    !settlement.encoder.trades().iter().any(|trade| {
        token_list
            .get(&trade.order.order_creation.sell_token)
            .is_none()
    })
}

fn print_settlements(rated_settlements: &[(Arc<dyn Solver>, RatedSettlement)]) {
    tracing::info!(
        "Rated Settlements: {:?}",
        rated_settlements
            .iter()
            .rev()
            .map(|(solver, settlement)| format!(
                "{}: objective={:.2e}: surplus={:.2e}, gas_estimate={:.2e}, gas_price={:.2e}",
                solver.name(),
                settlement.objective_value().to_f64().unwrap_or(f64::NAN),
                settlement.surplus.to_f64().unwrap_or(f64::NAN),
                settlement.gas_estimate.to_f64_lossy(),
                settlement.gas_price.to_f64().unwrap_or(f64::NAN),
            ))
            .collect::<Vec<_>>()
    );
}

#[derive(Debug)]
enum SolverRunError {
    Timeout,
    Solving(anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::Trade;
    use maplit::hashmap;
    use model::order::{Order, OrderCreation};
    use shared::token_list::Token;

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

        let trade = |token| Trade {
            order: Order {
                order_creation: OrderCreation {
                    sell_token: token,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let settlement = Settlement::with_trades(
            HashMap::new(),
            vec![trade(good_token), trade(another_good_token)],
        );
        assert!(is_only_selling_trusted_tokens(&settlement, &token_list));

        let settlement = Settlement::with_trades(
            HashMap::new(),
            vec![
                trade(good_token),
                trade(another_good_token),
                trade(bad_token),
            ],
        );
        assert!(!is_only_selling_trusted_tokens(&settlement, &token_list));
    }
}
