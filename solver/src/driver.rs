pub mod solver_settlements;

use self::solver_settlements::{RatedSettlement, SettlementWithSolver};
use crate::{
    chain,
    liquidity::{offchain_orderbook::BUY_ETH_ADDRESS, Liquidity},
    liquidity_collector::LiquidityCollector,
    metrics::SolverMetrics,
    settlement::Settlement,
    settlement_simulation, settlement_submission,
    solver::Solver,
};
use anyhow::{Context, Error, Result};
use contracts::GPv2Settlement;
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use itertools::{Either, Itertools};
use num::BigRational;
use primitive_types::H160;
use shared::{price_estimate::PriceEstimating, Web3};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

// There is no economic viability calculation yet so we're using an arbitrary very high cap to
// protect against a gas estimator giving bogus results that would drain all our funds.
const GAS_PRICE_CAP: f64 = 500e9;

pub struct Driver {
    settlement_contract: GPv2Settlement,
    liquidity_collector: LiquidityCollector,
    price_estimator: Arc<dyn PriceEstimating>,
    solver: Vec<Box<dyn Solver>>,
    gas_price_estimator: Box<dyn GasPriceEstimating>,
    target_confirm_time: Duration,
    settle_interval: Duration,
    native_token: H160,
    min_order_age: Duration,
    metrics: Arc<dyn SolverMetrics>,
    web3: Web3,
    network_id: String,
    max_merged_settlements: usize,
}
impl Driver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settlement_contract: GPv2Settlement,
        liquidity_collector: LiquidityCollector,
        price_estimator: Arc<dyn PriceEstimating>,
        solver: Vec<Box<dyn Solver>>,
        gas_price_estimator: Box<dyn GasPriceEstimating>,
        target_confirm_time: Duration,
        settle_interval: Duration,
        native_token: H160,
        min_order_age: Duration,
        metrics: Arc<dyn SolverMetrics>,
        web3: Web3,
        network_id: String,
        max_merged_settlements: usize,
    ) -> Self {
        Self {
            settlement_contract,
            liquidity_collector,
            price_estimator,
            solver,
            gas_price_estimator,
            target_confirm_time,
            settle_interval,
            native_token,
            min_order_age,
            metrics,
            web3,
            network_id,
            max_merged_settlements,
        }
    }

    pub async fn run_forever(&mut self) -> ! {
        loop {
            match self.single_run().await {
                Ok(()) => tracing::debug!("single run finished ok"),
                Err(err) => tracing::error!("single run errored: {:?}", err),
            }
            tokio::time::delay_for(self.settle_interval).await;
        }
    }

    // Returns solver name and result.
    async fn run_solvers(
        &self,
        liquidity: Vec<Liquidity>,
        gas_price: f64,
    ) -> impl Iterator<Item = (&'static str, Result<Vec<Settlement>>)> {
        join_all(self.solver.iter().map(|solver| {
            let liquidity = liquidity.clone();
            let metrics = &self.metrics;
            async move {
                let start_time = Instant::now();
                let settlement = solver.solve(liquidity, gas_price).await;
                metrics.settlement_computed(solver.name(), start_time);
                (solver.name(), settlement)
            }
        }))
        .await
        .into_iter()
    }

    async fn submit_settlement(&self, rated_settlement: RatedSettlement) {
        let SettlementWithSolver { name, settlement } = rated_settlement.clone().settlement;
        let trades = settlement.trades().to_vec();
        match settlement_submission::submit(
            &self.settlement_contract,
            self.gas_price_estimator.as_ref(),
            self.target_confirm_time,
            GAS_PRICE_CAP,
            rated_settlement,
        )
        .await
        {
            Ok(_) => {
                trades
                    .iter()
                    .for_each(|trade| self.metrics.order_settled(&trade.order, name));
            }
            Err(err) => tracing::error!("Failed to submit settlement: {:?}", err,),
        }
    }

    async fn can_settle(&self, settlement: RatedSettlement) -> Result<bool> {
        let simulations = settlement_simulation::simulate_settlements(
            chain![settlement.into()],
            &self.settlement_contract,
            &self.web3,
            &self.network_id,
            settlement_simulation::Block::LatestWithoutTenderly,
        )
        .await
        .context("failed to simulate settlement")?;
        Ok(simulations[0].is_ok())
    }

    // Split settlements into successfully simulating ones and errors.
    async fn simulate_settlements(
        &self,
        settlements: Vec<SettlementWithSolver>,
    ) -> Result<(
        Vec<SettlementWithSolver>,
        Vec<(SettlementWithSolver, Error)>,
    )> {
        let simulations = settlement_simulation::simulate_settlements(
            settlements
                .iter()
                .map(|settlement| settlement.settlement.clone().into()),
            &self.settlement_contract,
            &self.web3,
            &self.network_id,
            settlement_simulation::Block::LatestWithoutTenderly,
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
        errors: Vec<(SettlementWithSolver, Error)>,
        current_block_during_liquidity_fetch: u64,
    ) {
        let simulations = match settlement_simulation::simulate_settlements(
            errors
                .iter()
                .map(|(settlement, _)| settlement.clone().into()),
            &self.settlement_contract,
            &self.web3,
            &self.network_id,
            settlement_simulation::Block::FixedWithTenderly(current_block_during_liquidity_fetch),
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

        for ((settlement, _previous_error), result) in errors.into_iter().zip(simulations) {
            let error_at_earlier_block = match result {
                Ok(()) => continue,
                Err(err) => err,
            };
            tracing::error!(
                "settlement simulation failed right before submission AND for block {} which was current when liquidity was fetched:\n{:?}",
                current_block_during_liquidity_fetch, error_at_earlier_block
            );
            // This is an additional debug log so that the log message doesn't get too long as
            // settlement information is recoverable through tenderly anyway.
            tracing::warn!("settlement failure for: \n{:#?}", settlement,);
        }
    }

    // Rate settlements, ignoring those for which the rating procedure failed.
    async fn rate_settlements(
        &self,
        settlements: Vec<SettlementWithSolver>,
        prices: &HashMap<H160, BigRational>,
    ) -> Vec<RatedSettlement> {
        use futures::stream::StreamExt;
        futures::stream::iter(settlements)
            .filter_map(|settlement| async {
                let surplus = settlement.settlement.total_surplus(prices);
                let gas_estimate = settlement_submission::estimate_gas(
                    &self.settlement_contract,
                    &settlement.settlement.clone().into(),
                )
                .await
                .ok()?;
                Some(RatedSettlement {
                    settlement,
                    surplus,
                    gas_estimate,
                })
            })
            .collect::<Vec<_>>()
            .await
    }

    pub async fn single_run(&mut self) -> Result<()> {
        tracing::debug!("starting single run");
        let liquidity = self.liquidity_collector.get_liquidity().await?;
        let current_block_during_liquidity_fetch = self
            .web3
            .eth()
            .block_number()
            .await
            .context("failed to get current block")?
            .as_u64();

        let estimated_prices =
            collect_estimated_prices(self.price_estimator.as_ref(), self.native_token, &liquidity)
                .await;
        let liquidity = liquidity_with_price(liquidity, &estimated_prices);
        self.metrics.liquidity_fetched(&liquidity);

        let gas_price = self
            .gas_price_estimator
            .estimate()
            .await
            .context("failed to estimate gas price")?;
        tracing::debug!("solving with gas price of {}", gas_price);

        let settlements = self
            .run_solvers(liquidity, gas_price)
            .await
            .filter_map(solver_settlements::filter_bad_settlements)
            .inspect(|(name, settlements)| {
                for settlement in settlements.iter() {
                    tracing::debug!("solver {} found solution:\n {:?}", name, settlement);
                }
            });

        let mut settlements = settlements
            .map(|(name, settlements)| {
                solver_settlements::merge_settlements(
                    self.max_merged_settlements,
                    &estimated_prices,
                    name,
                    settlements,
                )
            })
            .flat_map(|settlements| -> Vec<SettlementWithSolver> { settlements.into() })
            .collect::<Vec<_>>();

        solver_settlements::filter_settlements_without_old_orders(
            self.min_order_age,
            &mut settlements,
        );

        let (settlements, errors) = self.simulate_settlements(settlements).await?;
        tracing::info!(
            "{} settlements passed simulation and {} failed",
            settlements.len(),
            errors.len()
        );
        for settlement in &settlements {
            self.metrics
                .settlement_simulation_succeeded(settlement.name);
        }
        for (settlement, _) in &errors {
            self.metrics.settlement_simulation_failed(settlement.name);
        }

        let rated_settlements = self.rate_settlements(settlements, &estimated_prices).await;

        if let Some(mut settlement) = rated_settlements.into_iter().max_by(|a, b| {
            a.objective_value(gas_price)
                .cmp(&b.objective_value(gas_price))
        }) {
            // If we have enough buffer in the settlement contract to not use on-chain interactions, remove those
            if self
                .can_settle(settlement.without_onchain_liquidity())
                .await
                .unwrap_or(false)
            {
                settlement = settlement.without_onchain_liquidity();
                tracing::info!("settlement without onchain liquidity");
            }
            self.submit_settlement(settlement).await;
        }

        // Happens after settlement submission so that we do not delay it.
        self.report_simulation_errors(errors, current_block_during_liquidity_fetch)
            .await;

        Ok(())
    }
}

pub async fn collect_estimated_prices(
    price_estimator: &dyn PriceEstimating,
    native_token: H160,
    liquidity: &[Liquidity],
) -> HashMap<H160, BigRational> {
    // Computes set of traded tokens (limit orders only).
    let mut tokens = HashSet::new();
    for liquid in liquidity {
        if let Liquidity::Limit(limit_order) = liquid {
            tokens.insert(limit_order.sell_token);
            tokens.insert(limit_order.buy_token);
        }
    }
    let tokens = tokens.drain().collect::<Vec<_>>();

    // For ranking purposes it doesn't matter how the external price vector is scaled,
    // but native_token is used here anyway for better logging/debugging.
    let denominator_token: H160 = native_token;

    let estimated_prices = price_estimator
        .estimate_prices(&tokens, denominator_token)
        .await;

    let mut prices: HashMap<_, _> = tokens
        .into_iter()
        .zip(estimated_prices)
        .filter_map(|(token, price)| match price {
            Ok(price) => Some((token, price)),
            Err(err) => {
                tracing::warn!("failed to estimate price for token {}: {:?}", token, err);
                None
            }
        })
        .collect();

    // If the wrapped native token is in the price list (e.g. WETH), so should be the placeholder for its native counterpart
    if let Some(price) = prices.get(&native_token).cloned() {
        prices.insert(BUY_ETH_ADDRESS, price);
    }
    prices
}

// Filter limit orders for which we don't have price estimates as they cannot be considered for the objective criterion
fn liquidity_with_price(
    liquidity: Vec<Liquidity>,
    prices: &HashMap<H160, BigRational>,
) -> Vec<Liquidity> {
    let (liquidity, removed_orders): (Vec<_>, Vec<_>) =
        liquidity
            .into_iter()
            .partition(|liquidity| match liquidity {
                Liquidity::Limit(limit_order) => [limit_order.sell_token, limit_order.buy_token]
                    .iter()
                    .all(|token| prices.contains_key(token)),
                Liquidity::Amm(_) => true,
            });
    if !removed_orders.is_empty() {
        tracing::debug!(
            "pruned {} orders: {:?}",
            removed_orders.len(),
            removed_orders,
        );
    }
    liquidity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::{tests::CapturingSettlementHandler, AmmOrder, LimitOrder};
    use maplit::hashmap;
    use model::{order::OrderKind, TokenPair};
    use num::{rational::Ratio, traits::One};
    use shared::price_estimate::mocks::{FailingPriceEstimator, FakePriceEstimator};

    #[tokio::test]
    async fn collect_estimated_prices_adds_prices_for_buy_and_sell_token_of_limit_orders() {
        let price_estimator = FakePriceEstimator(BigRational::from_float(1.0).unwrap());

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let liquidity = vec![
            Liquidity::Limit(LimitOrder {
                sell_amount: 100_000.into(),
                buy_amount: 100_000.into(),
                sell_token,
                buy_token,
                kind: OrderKind::Buy,
                partially_fillable: false,
                fee_amount: Default::default(),
                settlement_handling: CapturingSettlementHandler::arc(),
                id: "0".into(),
            }),
            Liquidity::Amm(AmmOrder {
                tokens: TokenPair::new(buy_token, native_token).unwrap(),
                reserves: (1_000_000, 1_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            }),
        ];
        let prices = collect_estimated_prices(&price_estimator, native_token, &liquidity).await;
        assert_eq!(prices.len(), 2);
        assert!(prices.contains_key(&sell_token));
        assert!(prices.contains_key(&buy_token));
    }

    #[tokio::test]
    async fn collect_estimated_prices_skips_token_for_which_estimate_fails() {
        let price_estimator = FailingPriceEstimator();

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let liquidity = vec![Liquidity::Limit(LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
        })];
        let prices = collect_estimated_prices(&price_estimator, native_token, &liquidity).await;
        assert_eq!(prices.len(), 0);
    }

    #[tokio::test]
    async fn collect_estimated_prices_adds_native_token_if_wrapped_is_traded() {
        let price_estimator = FakePriceEstimator(BigRational::from_float(1.0).unwrap());

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);

        let liquidity = vec![Liquidity::Limit(LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token: native_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
        })];
        let prices = collect_estimated_prices(&price_estimator, native_token, &liquidity).await;
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
        let order = |sell_token, buy_token| {
            Liquidity::Limit(LimitOrder {
                sell_token,
                buy_token,
                ..Default::default()
            })
        };
        let liquidity = vec![
            order(tokens[0], tokens[1]),
            order(tokens[0], tokens[2]),
            order(tokens[2], tokens[0]),
            order(tokens[2], tokens[3]),
        ];
        let filtered = liquidity_with_price(liquidity, &prices);
        assert_eq!(filtered.len(), 1);
        assert!(
            matches!(&filtered[0], Liquidity::Limit(order) if order.sell_token == tokens[0] && order.buy_token == tokens[1])
        );
    }
}
