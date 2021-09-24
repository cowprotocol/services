pub mod solver_settlements;

use self::solver_settlements::RatedSettlement;
use crate::{
    liquidity::LimitOrder,
    liquidity_collector::LiquidityCollector,
    metrics::SolverMetrics,
    settlement::Settlement,
    settlement_simulation,
    settlement_submission::{self, retry::is_transaction_failure, SolutionSubmitter},
    solver::Solver,
    solver::{Auction, SettlementWithSolver, Solvers},
};
use anyhow::{anyhow, Context, Error, Result};
use contracts::GPv2Settlement;
use ethcontract::errors::MethodError;
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use itertools::{Either, Itertools};
use model::order::{OrderUid, BUY_ETH_ADDRESS};
use num::BigRational;
use primitive_types::{H160, U256};
use shared::{
    current_block::{self, CurrentBlockStream},
    price_estimate,
    price_estimate::PriceEstimating,
    recent_block_cache::Block,
    token_list::TokenList,
    Web3,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

type IntermediateToupleOfVecOfSettlements = (
    Vec<Vec<SettlementWithSolver>>,
    Vec<Vec<(SettlementWithSolver, Error)>>,
);

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
    inflight_trades: HashSet<OrderUid>,
    block_stream: CurrentBlockStream,
    fee_factor: f64,
    solution_submitter: SolutionSubmitter,
    solve_id: u64,
    native_token_amount_to_estimate_prices_with: U256,
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
        fee_factor: f64,
        solution_submitter: SolutionSubmitter,
        native_token_amount_to_estimate_prices_with: U256,
    ) -> Self {
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
            inflight_trades: HashSet::new(),
            block_stream,
            fee_factor,
            solution_submitter,
            solve_id: 0,
            native_token_amount_to_estimate_prices_with,
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
    ) -> Vec<(Arc<dyn Solver>, Result<Vec<Settlement>>)> {
        join_all(self.solvers.iter().map(|solver| {
            let auction = auction.clone();
            let metrics = &self.metrics;
            async move {
                let start_time = Instant::now();
                let result =
                    match tokio::time::timeout_at(auction.deadline.into(), solver.solve(auction))
                        .await
                    {
                        Ok(inner) => inner,
                        Err(_timeout) => Err(anyhow!("solver timed out")),
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
    ) -> Result<()> {
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
            Ok(hash) => {
                let name = solver.name();
                tracing::info!("Successfully submitted {} settlement: {:?}", name, hash);
                trades
                    .iter()
                    .for_each(|trade| self.metrics.order_settled(&trade.order, name));
                self.metrics.settlement_submitted(true, name);
                Ok(())
            }
            Err(err) => {
                // Since we simulate and only submit solutions when they used to pass before, there is no
                // point in logging transaction failures in the form of race conditions as hard errors.
                let name = solver.name();
                if err
                    .downcast_ref::<MethodError>()
                    .map(|e| is_transaction_failure(&e.inner))
                    .unwrap_or(false)
                {
                    tracing::warn!("Failed to submit {} settlement: {:?}", name, err)
                } else {
                    tracing::error!("Failed to submit {} settlement: {:?}", name, err)
                };
                self.metrics.settlement_submitted(false, name);
                Err(err)
            }
        }
    }

    async fn can_settle_without_liquidity(
        &self,
        solver: Arc<dyn Solver>,
        settlement: &RatedSettlement,
        gas_price_wei: f64,
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

        let simulations = settlement_simulation::simulate_settlements(
            [(solver, settlement.settlement.without_onchain_liquidity())].iter(),
            &self.settlement_contract,
            &self.web3,
            &self.network_id,
            settlement_simulation::Block::LatestWithoutTenderly,
            gas_price_wei,
        )
        .await
        .context("failed to simulate settlement")?;
        Ok(simulations[0].is_ok())
    }

    // Split settlements into successfully simulating ones and errors.
    async fn simulate_settlements(
        &self,
        settlements: Vec<SettlementWithSolver>,
        gas_price_wei: f64,
    ) -> Result<(
        Vec<SettlementWithSolver>,
        Vec<(SettlementWithSolver, Error)>,
    )> {
        let simulations = settlement_simulation::simulate_settlements(
            settlements.iter(),
            &self.settlement_contract,
            &self.web3,
            &self.network_id,
            settlement_simulation::Block::LatestWithoutTenderly,
            gas_price_wei,
        )
        .await
        .context("failed to simulate settlements")?;

        Ok(settlements
            .into_iter()
            .zip(simulations)
            .partition_map(|(settlement, result)| match result {
                Ok(()) => Either::Left(settlement),
                Err(err) => Either::Right((settlement, err)),
            }))
    }

    // Log simulation errors only if the simulation also fails in the block at which on chain
    // liquidity was queried. If the simulation succeeds at the previous block then the solver
    // worked correctly and the error doesn't have to be reported.
    // Note that we could still report a false positive because the earlier block might be off by if
    // the block has changed just as were were querying the node.
    async fn report_simulation_errors(
        &self,
        errors: &[(SettlementWithSolver, Error)],
        current_block_during_liquidity_fetch: u64,
        gas_price_wei: f64,
    ) {
        let simulations = match settlement_simulation::simulate_settlements(
            errors.iter().map(|(settlement, _)| settlement),
            &self.settlement_contract,
            &self.web3,
            &self.network_id,
            settlement_simulation::Block::FixedWithTenderly(current_block_during_liquidity_fetch),
            gas_price_wei,
        )
        .await
        {
            Ok(simulations) => simulations,
            Err(err) => {
                tracing::error!(
                    "unable to complete simulation of settlements at earlier block {}: {:?}",
                    current_block_during_liquidity_fetch,
                    err
                );
                return;
            }
        };

        for (((solver, settlement), _previous_error), result) in errors.iter().zip(simulations) {
            self.metrics
                .settlement_simulation_failed_on_latest(solver.name());
            if let Err(error_at_earlier_block) = result {
                tracing::warn!(
                    "{} settlement simulation failed at submission and block {}:\n{:?}",
                    solver.name(),
                    current_block_during_liquidity_fetch,
                    error_at_earlier_block,
                );
                // split warning into separate logs so that the messages aren't too long.
                tracing::warn!("settlement failure for: \n{:#?}", settlement);

                self.metrics.settlement_simulation_failed(solver.name());
            }
        }
    }

    // Record metric with the amount of orders that were matched but not settled in this runloop (effectively queued for the next one)
    // Should help us to identify how much we can save by parallelizing execution.
    fn report_matched_but_unsettled_orders(
        &self,
        submitted: &Settlement,
        all: impl Iterator<Item = RatedSettlement>,
    ) {
        let submitted: HashSet<_> = submitted
            .trades()
            .iter()
            .map(|trade| trade.order.order_meta_data.uid)
            .collect();
        let all_matched: HashSet<_> = all
            .flat_map(|solution| solution.settlement.trades().to_vec())
            .map(|trade| trade.order.order_meta_data.uid)
            .collect();
        let matched_but_not_settled: HashSet<_> = all_matched.difference(&submitted).collect();
        self.metrics
            .orders_matched_but_not_settled(matched_but_not_settled.len())
    }

    // Rate settlements, ignoring those for which the rating procedure failed.
    async fn rate_settlements(
        &self,
        settlements: Vec<SettlementWithSolver>,
        prices: &HashMap<H160, BigRational>,
        gas_price_wei: f64,
    ) -> Vec<(Arc<dyn Solver>, RatedSettlement)> {
        use futures::stream::StreamExt;

        // Normalize gas_price_wei to the native token price in the prices vector.
        let gas_price_wei = BigRational::from_float(gas_price_wei).expect("Invalid gas price.")
            * prices
                .get(&self.native_token)
                .expect("Price of native token must be known.");

        futures::stream::iter(settlements)
            .filter_map(|(solver, settlement)| async {
                let surplus = settlement.total_surplus(prices);
                // Because of a potential fee discount, the solver fees may by themselves not be sufficient to make a solution economically viable (leading to a negative objective value)
                // We therefore reverse apply the fee discount to simulate unsubsidized fees for ranking.
                let unsubsidized_solver_fees = settlement.total_fees(prices) / BigRational::from_float(self.fee_factor).expect("Discount factor is not a rational");
                let gas_estimate = settlement_submission::estimate_gas(
                    &self.settlement_contract,
                    &settlement.clone().into(),
                    solver.account().clone(),
                )
                .await
                .map_err(|err| {
                    tracing::error!("Failed to estimate gas for solver {}: {:?}", solver.name(), err);
                    err
                })
                .ok()?;
                let rated_settlement = RatedSettlement {
                    settlement,
                    surplus,
                    solver_fees: unsubsidized_solver_fees,
                    gas_estimate,
                    gas_price: gas_price_wei.clone(),
                };
                tracing::info!(
                    "Objective value for solver {} is {}: surplus={}, gas_estimate={}, gas_price={}",
                    solver.name(),
                    rated_settlement.objective_value(),
                    rated_settlement.surplus,
                    rated_settlement.gas_estimate,
                    rated_settlement.gas_price,
                );
                Some((solver, rated_settlement))
            })
            .collect::<Vec<_>>()
            .await
    }

    // Takes the settlements of a single solver and adds a merged settlement.
    pub async fn merge_settlements(
        &self,
        solver: Arc<dyn Solver>,
        max_merged_settlements: usize,
        prices: &HashMap<H160, BigRational>,
        settlements: &mut Vec<Settlement>,
        gas_price_wei: f64,
    ) -> Result<(
        Option<SettlementWithSolver>,
        Vec<(SettlementWithSolver, Error)>,
    )> {
        settlements.sort_by_cached_key(|a| -a.total_surplus(prices));
        self.merge_at_most_settlements(
            solver.clone(),
            max_merged_settlements,
            settlements.clone().into_iter(),
            gas_price_wei,
        )
        .await
    }

    // Goes through the settlements in order and tries to merge a number of them. Keeps going on merge
    // error.
    async fn merge_at_most_settlements(
        &self,
        solver: Arc<dyn Solver>,
        max_merges: usize,
        mut settlements: impl Iterator<Item = Settlement>,
        gas_price_wei: f64,
    ) -> Result<(
        Option<SettlementWithSolver>,
        Vec<(SettlementWithSolver, Error)>,
    )> {
        let mut simulation_errors = Vec::new();
        let mut initial_settelment = match settlements.next() {
            Some(settlement) => settlement,
            None => return Ok((None, simulation_errors)),
        };
        loop {
            // one can also use the following pattern: let (settlements, errors) =self.simulate().await?;
            let (successfully_simulated_settlements, mut errors) = self
                .simulate_settlements(
                    vec![(solver.clone(), initial_settelment.clone())],
                    gas_price_wei,
                )
                .await?;
            if !successfully_simulated_settlements.is_empty() {
                break;
            }
            simulation_errors.append(&mut errors);
            initial_settelment = match settlements.next() {
                Some(settlement) => settlement,
                _ => return Ok((None, simulation_errors)),
            };
        }
        let mut merged = initial_settelment;
        let mut merge_count = 1;
        while merge_count < max_merges {
            let next = match settlements.next() {
                Some(settlement) => settlement,
                None => break,
            };
            let proposed_settlement = match merged.clone().merge(next) {
                Ok(settlement) => settlement,
                Err(err) => {
                    tracing::debug!("failed to merge settlement: {:?}", err);
                    continue;
                }
            };
            let (successfully_simulated_settlements, mut last_merge_error) = self
                .simulate_settlements(
                    vec![(solver.clone(), proposed_settlement.clone())],
                    gas_price_wei,
                )
                .await?;
            simulation_errors.append(&mut last_merge_error);

            if let Some((_, settlements)) = successfully_simulated_settlements.first() {
                merged = settlements.clone();
                merge_count += 1;
            }
        }
        Ok((Some((solver.clone(), merged.clone())), simulation_errors))
    }

    pub async fn single_run(&mut self) -> Result<()> {
        tracing::debug!("starting single run");
        let current_block_during_liquidity_fetch =
            current_block::block_number(&self.block_stream.borrow())?;

        let orders = self
            .liquidity_collector
            .get_orders(&self.inflight_trades)
            .await?;
        let liquidity = self
            .liquidity_collector
            .get_liquidity_for_orders(&orders, Block::Number(current_block_during_liquidity_fetch))
            .await?;

        let estimated_prices = collect_estimated_prices(
            self.price_estimator.as_ref(),
            self.native_token_amount_to_estimate_prices_with,
            self.native_token,
            &orders,
        )
        .await;
        tracing::debug!("estimated prices: {:?}", estimated_prices);

        let orders = orders_with_price_estimates(orders, &estimated_prices);

        self.metrics.orders_fetched(&orders);
        self.metrics.liquidity_fetched(&liquidity);

        let gas_price_wei = self
            .gas_price_estimator
            .estimate()
            .await
            .context("failed to estimate gas price")?;
        tracing::debug!("solving with gas price of {}", gas_price_wei);

        let auction = Auction {
            id: self.next_auction_id(),
            orders,
            liquidity,
            gas_price: gas_price_wei,
            deadline: Instant::now() + self.solver_time_limit,
            price_estimates: estimated_prices.clone(),
        };
        tracing::debug!("solving auction ID {}", auction.id);

        let run_solver_results = self.run_solvers(auction).await;
        let solver_settlements = futures::future::join_all(run_solver_results.into_iter().map(
            |(solver, settelment_result)| {
                self.process_solver_solutions(
                    solver,
                    settelment_result,
                    &estimated_prices,
                    gas_price_wei,
                )
            },
        ))
        .await;
        let (settlements, errors): IntermediateToupleOfVecOfSettlements = solver_settlements
            .into_iter()
            .filter_map(|result| result.ok())
            .map(|(settlement, errors)| match settlement {
                Some(settlement) => (vec![settlement], errors),
                None => (vec![], errors),
            })
            .unzip();
        let mut settlements: Vec<SettlementWithSolver> =
            settlements.into_iter().flatten().collect();

        solver_settlements::filter_settlements_without_old_orders(
            self.min_order_age,
            &mut settlements,
        );
        let errors: Vec<(SettlementWithSolver, Error)> = errors.into_iter().flatten().collect();
        tracing::info!(
            "{} settlements passed simulation and {} failed",
            settlements.len(),
            errors.len()
        );
        for (solver, _) in &settlements {
            self.metrics.settlement_simulation_succeeded(solver.name());
        }

        let rated_settlements = self
            .rate_settlements(settlements, &estimated_prices, gas_price_wei)
            .await;

        self.inflight_trades.clear();
        if let Some((solver, mut settlement)) = rated_settlements
            .clone()
            .into_iter()
            .max_by(|a, b| a.1.objective_value().cmp(&b.1.objective_value()))
        {
            // If we have enough buffer in the settlement contract to not use on-chain interactions, remove those
            if self
                .can_settle_without_liquidity(solver.clone(), &settlement, gas_price_wei)
                .await
                .unwrap_or(false)
            {
                settlement.settlement = settlement.settlement.without_onchain_liquidity();
                tracing::info!("settlement without onchain liquidity");
            }

            tracing::info!("winning settlement: {:?}", settlement);
            if self
                .submit_settlement(solver, settlement.clone())
                .await
                .is_ok()
            {
                self.inflight_trades = settlement
                    .settlement
                    .trades()
                    .iter()
                    .map(|t| t.order.order_meta_data.uid)
                    .collect::<HashSet<OrderUid>>();
            }

            self.report_matched_but_unsettled_orders(
                &settlement.settlement,
                rated_settlements.into_iter().map(|(_, solution)| solution),
            );
        }

        // Happens after settlement submission so that we do not delay it.
        self.report_simulation_errors(&errors, current_block_during_liquidity_fetch, gas_price_wei)
            .await;

        Ok(())
    }

    fn next_auction_id(&mut self) -> u64 {
        let id = self.solve_id;
        self.solve_id += 1;
        id
    }

    async fn process_solver_solutions(
        &self,
        solver: Arc<dyn Solver>,
        settlements: Result<Vec<Settlement>>,
        estimated_prices: &HashMap<H160, BigRational>,
        gas_price_wei: f64,
    ) -> Result<(
        Option<SettlementWithSolver>,
        Vec<(SettlementWithSolver, Error)>,
    )> {
        let name = solver.name();
        let mut settlements = match settlements {
            Ok(settlement) => settlement,
            Err(err) => {
                return Err(anyhow!("solver {} error: {:?}", name, err));
            }
        };

        solver_settlements::filter_empty_settlements(&mut settlements);

        for settlement in &settlements {
            tracing::debug!("solver {} found solution:\n{:?}", name, settlement);
        }

        self.merge_settlements(
            solver.clone(),
            self.max_merged_settlements,
            estimated_prices,
            &mut settlements,
            gas_price_wei,
        )
        .await
    }
}

pub async fn collect_estimated_prices(
    price_estimator: &dyn PriceEstimating,
    native_token_amount_to_estimate_prices_with: U256,
    native_token: H160,
    orders: &[LimitOrder],
) -> HashMap<H160, BigRational> {
    // Computes set of traded tokens (limit orders only).
    // NOTE: The native token is always added.

    let queries = orders
        .iter()
        .flat_map(|order| [order.sell_token, order.buy_token])
        .filter(|token| *token != native_token)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|token| price_estimate::Query {
            // For ranking purposes it doesn't matter how the external price vector is scaled,
            // but native_token is used here anyway for better logging/debugging.
            sell_token: native_token,
            buy_token: token,
            in_amount: native_token_amount_to_estimate_prices_with,
            kind: model::order::OrderKind::Sell,
        })
        .collect::<Vec<_>>();
    let estimates = price_estimator.estimates(&queries).await;

    fn log_err(token: H160, err: &str) {
        tracing::warn!("failed to estimate price for token {}: {}", token, err);
    }
    let mut prices: HashMap<_, _> = queries
        .into_iter()
        .zip(estimates)
        .filter_map(|(query, estimate)| {
            let estimate = match estimate {
                Ok(estimate) => estimate,
                Err(err) => {
                    log_err(query.buy_token, &format!("{:?}", err));
                    return None;
                }
            };
            let price = match estimate.price_in_sell_token_rational(&query) {
                Some(price) => price,
                None => {
                    log_err(query.buy_token, "infinite price");
                    return None;
                }
            };
            Some((query.buy_token, price))
        })
        .collect();

    // Always include the native token.
    prices.insert(native_token, num::one());
    // And the placeholder for its native counterpart.
    prices.insert(BUY_ETH_ADDRESS, num::one());

    prices
}

// Filter limit orders for which we don't have price estimates as they cannot be considered for the objective criterion
fn orders_with_price_estimates(
    orders: Vec<LimitOrder>,
    prices: &HashMap<H160, BigRational>,
) -> Vec<LimitOrder> {
    let (orders, removed_orders): (Vec<_>, Vec<_>) = orders.into_iter().partition(|order| {
        prices.contains_key(&order.sell_token) && prices.contains_key(&order.buy_token)
    });
    if !removed_orders.is_empty() {
        tracing::debug!(
            "pruned {} orders: {:?}",
            removed_orders.len(),
            removed_orders,
        );
    }
    orders
}

fn is_only_selling_trusted_tokens(settlement: &Settlement, token_list: &TokenList) -> bool {
    !settlement.encoder.trades().iter().any(|trade| {
        token_list
            .get(&trade.order.order_creation.sell_token)
            .is_none()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        liquidity::{tests::CapturingSettlementHandler, LimitOrder},
        settlement::Trade,
    };
    use maplit::hashmap;
    use model::order::{Order, OrderCreation, OrderKind};
    use num::rational::BigRational;
    use num::traits::One as _;
    use shared::{
        price_estimate::mocks::{FailingPriceEstimator, FakePriceEstimator},
        token_list::Token,
    };

    #[tokio::test]
    async fn collect_estimated_prices_adds_prices_for_buy_and_sell_token_of_limit_orders() {
        let price_estimator = FakePriceEstimator(price_estimate::Estimate {
            out_amount: 1.into(),
            gas: 1.into(),
        });

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let orders = vec![LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
            is_liquidity_order: false,
        }];
        let prices =
            collect_estimated_prices(&price_estimator, 1.into(), native_token, &orders).await;
        assert_eq!(prices.len(), 4);
        assert!(prices.contains_key(&sell_token));
        assert!(prices.contains_key(&buy_token));
    }

    #[tokio::test]
    async fn collect_estimated_prices_skips_token_for_which_estimate_fails() {
        let price_estimator = FailingPriceEstimator();

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let orders = vec![LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
            is_liquidity_order: false,
        }];
        let prices =
            collect_estimated_prices(&price_estimator, 1.into(), native_token, &orders).await;
        assert_eq!(prices.len(), 2);
    }

    #[tokio::test]
    async fn collect_estimated_prices_adds_native_token_if_wrapped_is_traded() {
        let price_estimator = FakePriceEstimator(price_estimate::Estimate {
            out_amount: 1.into(),
            gas: 1.into(),
        });

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);

        let liquidity = vec![LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token: native_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
            is_liquidity_order: false,
        }];
        let prices =
            collect_estimated_prices(&price_estimator, 1.into(), native_token, &liquidity).await;
        assert_eq!(prices.len(), 3);
        assert!(prices.contains_key(&sell_token));
        assert!(prices.contains_key(&native_token));
        assert!(prices.contains_key(&BUY_ETH_ADDRESS));
    }

    #[test]
    fn liquidity_with_price_removes_liquidity_without_price() {
        let tokens = [
            H160::from_low_u64_be(0),
            H160::from_low_u64_be(1),
            H160::from_low_u64_be(2),
            H160::from_low_u64_be(3),
        ];
        let prices = hashmap! {tokens[0] => BigRational::one(), tokens[1] => BigRational::one()};
        let order = |sell_token, buy_token| LimitOrder {
            sell_token,
            buy_token,
            ..Default::default()
        };
        let orders = vec![
            order(tokens[0], tokens[1]),
            order(tokens[0], tokens[2]),
            order(tokens[2], tokens[0]),
            order(tokens[2], tokens[3]),
        ];
        let filtered = orders_with_price_estimates(orders, &prices);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].sell_token == tokens[0] && filtered[0].buy_token == tokens[1]);
    }

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
    // #[test]
    // fn merges_settlements_with_highest_objective_value() {
    //     let token0 = H160::from_low_u64_be(0);
    //     let token1 = H160::from_low_u64_be(1);
    //     let prices = hashmap! { token0 => 1.into(), token1 => 1.into()};
    //     let prices_rational = hashmap! {
    //         token0 => BigRational::from_u8(1).unwrap(),
    //         token1 => BigRational::from_u8(1).unwrap()
    //     };
    //     fn uid(number: u8) -> OrderUid {
    //         OrderUid([number; 56])
    //     }

    //     let trade = |executed_amount, uid_: u8| Trade {
    //         sell_token_index: 0,
    //         buy_token_index: 1,
    //         executed_amount,
    //         order: Order {
    //             order_meta_data: OrderMetaData {
    //                 uid: uid(uid_),
    //                 ..Default::default()
    //             },
    //             order_creation: OrderCreation {
    //                 sell_token: token0,
    //                 buy_token: token1,
    //                 sell_amount: executed_amount,
    //                 buy_amount: 1.into(),
    //                 kind: OrderKind::Buy,
    //                 ..Default::default()
    //             },
    //         },
    //     };
    //     let settlement = |executed_amount: U256, order_uid: u8| {
    //         Settlement::with_trades(prices.clone(), vec![trade(executed_amount, order_uid)])
    //     };

    //     let mut settlements = vec![
    //         settlement(1.into(), 1),
    //         settlement(2.into(), 2),
    //         settlement(3.into(), 3),
    //     ];
    //     merge_settlements(2, &prices_rational, &mut settlements);

    //     assert_eq!(settlements.len(), 4);
    //     assert!(settlements.iter().any(|settlement| {
    //         let trades = settlement.trades();
    //         let uids: HashSet<OrderUid> = trades
    //             .iter()
    //             .map(|trade| trade.order.order_meta_data.uid)
    //             .collect();
    //         uids.len() == 2 && uids.contains(&uid(2)) && uids.contains(&uid(3))
    //     }));
    // }

    // #[test]
    // fn merge_continues_on_error() {
    //     let token0 = H160::from_low_u64_be(0);
    //     let token1 = H160::from_low_u64_be(1);
    //     let settlement0 = Settlement::new(hashmap! {token0 => 1.into(), token1 => 2.into()});
    //     let settlement1 = Settlement::new(hashmap! {token0 => 2.into(), token1 => 2.into()});
    //     let settlement2 = Settlement::new(hashmap! {token0 => 1.into(), token1 => 2.into()});
    //     let settlements = vec![settlement0, settlement1, settlement2];

    //     // Can't merge 0 with 1 because token0 and token1 clearing prices are different.
    //     let merged = merge_at_most_settlements(2, settlements.into_iter()).unwrap();
    //     assert_eq!(merged.clearing_price(token0), Some(1.into()));
    //     assert_eq!(merged.clearing_price(token1), Some(2.into()));
    // }

    // #[test]
    // fn merge_does_nothing_on_max_1_merge() {
    //     let token0 = H160::from_low_u64_be(0);
    //     let settlement = Settlement::new(hashmap! {token0 => 0.into()});
    //     let settlements = vec![settlement.clone(), settlement];
    //     assert!(merge_at_most_settlements(1, settlements.into_iter()).is_none());
    // }
}
