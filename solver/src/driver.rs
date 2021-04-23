use crate::{
    liquidity::{offchain_orderbook::BUY_ETH_ADDRESS, Liquidity},
    liquidity_collector::LiquidityCollector,
    metrics::SolverMetrics,
    settlement::Settlement,
    settlement_submission,
    solver::Solver,
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use num::BigRational;
use primitive_types::H160;
use shared::price_estimate::PriceEstimating;
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

    async fn run_solvers(
        &self,
        liquidity: Vec<Liquidity>,
        gas_price: f64,
        prices: &HashMap<H160, BigRational>,
    ) -> Vec<SolverSettlement> {
        join_all(self.solver.iter().enumerate().map(|(index, solver)| {
            let liquidity = liquidity.clone();
            let metrics = &self.metrics;
            async move {
                let start_time = Instant::now();
                let settlement = solver.solve(liquidity, gas_price).await;
                metrics.settlement_computed(solver.to_string().as_str(), start_time);
                (index, settlement)
            }
        }))
        .await
        .into_iter()
        .filter_map(|(index, settlement)| {
            let settlement = match settlement {
                Ok(settlement) => settlement,
                Err(err) => {
                    tracing::error!("solver {} error: {:?}", self.solver[index].to_string(), err);
                    return None;
                }
            }?;
            if settlement.trades().is_empty() {
                return None;
            }
            let objective_value = settlement.objective_value(prices);
            Some(SolverSettlement {
                index,
                settlement,
                objective_value,
            })
        })
        .collect()
    }

    // Go through the settlements in order until the first success.
    async fn submit_settlements(&self, settlements: Vec<SolverSettlement>) {
        let settle_orders_older_than =
            chrono::offset::Utc::now() - chrono::Duration::from_std(self.min_order_age).unwrap();
        for settlement in settlements {
            // If all orders are younger than self.min_order_age skip settlement. Orders will still
            // be settled once they have been in the order book for longer. This makes coincidence
            // of wants more likely.
            let should_be_settled_immediately =
                settlement.settlement.trades().iter().any(|trade| {
                    trade.order.order_meta_data.creation_date <= settle_orders_older_than
                });
            if !should_be_settled_immediately {
                tracing::info!(
                    "Skipping settlement because no trade is older than {}s",
                    self.min_order_age.as_secs()
                );
                continue;
            }

            let trades = settlement.settlement.trades().to_vec();
            match settlement_submission::submit(
                &self.settlement_contract,
                self.gas_price_estimator.as_ref(),
                self.target_confirm_time,
                GAS_PRICE_CAP,
                settlement.settlement,
            )
            .await
            {
                Ok(_) => {
                    trades
                        .iter()
                        .for_each(|trade| self.metrics.order_settled(&trade.order));
                    break;
                }
                Err(err) => tracing::error!(
                    "{} Failed to submit settlement: {:?}",
                    self.solver[settlement.index],
                    err,
                ),
            }
        }
    }

    pub async fn single_run(&mut self) -> Result<()> {
        tracing::debug!("starting single run");
        let liquidity = self.liquidity_collector.get_liquidity().await?;

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

        let mut settlements = self
            .run_solvers(liquidity, gas_price, &estimated_prices)
            .await;
        // Sort by key in descending order. Reversed by doing b.cmp(a) instead of a.cmp(b).
        settlements.sort_unstable_by(|a, b| b.objective_value.cmp(&a.objective_value));

        for settlement in settlements.iter() {
            tracing::info!(
                "solver {} found solution with objective value {}: {:?}",
                self.solver[settlement.index],
                settlement.objective_value,
                settlement.settlement,
            );
        }

        self.submit_settlements(settlements).await;

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

struct SolverSettlement {
    // Index in the Driver::solver vector
    index: usize,
    settlement: Settlement,
    objective_value: BigRational,
}

#[cfg(test)]
mod tests {
    use shared::price_estimate::mocks::{FailingPriceEstimator, FakePriceEstimator};

    use super::*;
    use crate::liquidity::{tests::CapturingSettlementHandler, AmmOrder, LimitOrder};
    use model::{order::OrderKind, TokenPair};
    use num::rational::Ratio;

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
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
        })];
        let prices = collect_estimated_prices(&price_estimator, native_token, &liquidity).await;
        assert_eq!(prices.len(), 3);
        assert!(prices.contains_key(&sell_token));
        assert!(prices.contains_key(&native_token));
        assert!(prices.contains_key(&BUY_ETH_ADDRESS));
    }
}
