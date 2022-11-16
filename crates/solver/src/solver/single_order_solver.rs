use crate::{
    driver::solver_settlements::merge_settlements,
    liquidity::LimitOrder,
    metrics::SolverMetrics,
    settlement::{external_prices::ExternalPrices, Settlement},
    solver::{Auction, Solver},
};
use anyhow::{Error, Result};
use clap::Parser;
use ethcontract::Account;
use num::ToPrimitive;
use number_conversions::u256_to_big_rational;
use primitive_types::U256;
use rand::prelude::SliceRandom;
use std::{
    collections::VecDeque,
    fmt::{self, Display, Formatter},
    sync::Arc,
    time::Duration,
};

/// CLI arguments to configure order prioritization of single order solvers based on an orders price.
#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Arguments {
    /// Exponent to turn an order's price ratio into a weight for a weighted prioritization.
    #[clap(long, env, default_value = "10.0")]
    pub price_priority_exponent: f64,

    /// The lowest possible weight an order can have for the weighted order prioritization.
    #[clap(long, env, default_value = "0.01")]
    pub price_priority_min_weight: f64,

    /// The highest possible weight an order can have for the weighted order prioritization.
    #[clap(long, env, default_value = "10.0")]
    pub price_priority_max_weight: f64,
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(
            f,
            "price_priority_exponent: {}",
            self.price_priority_exponent
        )?;
        writeln!(
            f,
            "price_priority_min_weight: {}",
            self.price_priority_min_weight
        )?;
        writeln!(
            f,
            "price_priority_max_weight: {}",
            self.price_priority_max_weight
        )?;
        Ok(())
    }
}

impl Arguments {
    fn apply_weight_constraints(&self, original_weight: f64) -> f64 {
        original_weight
            .powf(self.price_priority_exponent)
            .max(self.price_priority_min_weight)
            .min(self.price_priority_max_weight)
    }
}

impl Default for Arguments {
    fn default() -> Self {
        // Arguments which seem to produce reasonable results for orders between 90% and
        // 130% of the market price.
        Self {
            price_priority_exponent: 10.,
            price_priority_min_weight: 0.01,
            price_priority_max_weight: 10.,
        }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
/// Implementations of this trait know how to settle a single limit order (not taking advantage of batching multiple orders together)
pub trait SingleOrderSolving: Send + Sync + 'static {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        auction: &Auction,
    ) -> Result<Option<Settlement>, SettlementError>;

    /// Solver's account that should be used to submit settlements.
    fn account(&self) -> &Account;

    /// Displayable name of the solver. Defaults to the type name.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub struct SingleOrderSolver {
    inner: Box<dyn SingleOrderSolving>,
    metrics: Arc<dyn SolverMetrics>,
    max_merged_settlements: usize,
    max_settlements_per_solver: usize,
    order_prioritization_config: Arguments,
}

impl SingleOrderSolver {
    pub fn new(
        inner: Box<dyn SingleOrderSolving>,
        metrics: Arc<dyn SolverMetrics>,
        max_settlements_per_solver: usize,
        max_merged_settlements: usize,
        order_prioritization_config: Arguments,
    ) -> Self {
        Self {
            inner,
            metrics,
            max_merged_settlements,
            max_settlements_per_solver,
            order_prioritization_config,
        }
    }
}

#[async_trait::async_trait]
impl Solver for SingleOrderSolver {
    async fn solve(&self, auction: Auction) -> Result<Vec<Settlement>> {
        let mut orders = get_prioritized_orders(
            &auction.orders,
            &auction.external_prices,
            &self.order_prioritization_config,
        );

        let mut settlements = Vec::new();
        let settle = async {
            while let Some(order) = orders.pop_front() {
                match self.inner.try_settle_order(order.clone(), &auction).await {
                    Ok(settlement) => {
                        self.metrics
                            .single_order_solver_succeeded(self.inner.name());
                        settlements.extend(settlement)
                    }
                    Err(err) => {
                        let name = self.inner.name();
                        self.metrics.single_order_solver_failed(name);
                        if err.retryable {
                            tracing::warn!("Solver {} retryable error: {:?}", name, &err.inner);
                            orders.push_back(order);
                        } else {
                            tracing::warn!("Solver {} error: {:?}", name, &err.inner);
                        }
                    }
                }
            }
        };

        // Subtract a small amount of time to ensure that the driver doesn't reach the deadline first.
        let _ = tokio::time::timeout_at((auction.deadline - Duration::from_secs(1)).into(), settle)
            .await;

        // Keep at most this many settlements. This is important in case where a solver produces
        // a large number of settlements which would hold up the driver logic when simulating
        // them.
        // Shuffle first so that in the case a buggy solver keeps returning some amount of
        // invalid settlements first we have a chance to make progress.
        settlements.shuffle(&mut rand::thread_rng());
        settlements.truncate(self.max_settlements_per_solver);

        merge_settlements(
            self.max_merged_settlements,
            &auction.external_prices,
            &mut settlements,
        );

        Ok(settlements)
    }

    fn account(&self) -> &Account {
        self.inner.account()
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }
}

#[derive(Debug)]
pub struct SettlementError {
    pub inner: anyhow::Error,
    pub retryable: bool,
}

impl From<anyhow::Error> for SettlementError {
    fn from(err: Error) -> Self {
        SettlementError {
            inner: err,
            retryable: false,
        }
    }
}

/// Returns the `native_sell_amount / native_buy_amount` of the given order under the current
/// market conditions. The higher the value the more likely it is that this order could get filled.
fn estimate_price_viability(order: &LimitOrder, prices: &ExternalPrices) -> f64 {
    let sell_amount = u256_to_big_rational(&order.sell_amount);
    let buy_amount = u256_to_big_rational(&order.buy_amount);
    let native_sell_amount = prices.get_native_amount(order.sell_token, sell_amount);
    let native_buy_amount = prices.get_native_amount(order.buy_token, buy_amount);
    (native_sell_amount / native_buy_amount)
        .to_f64()
        .unwrap_or(0.)
}

/// In case there are too many orders to solve before the auction deadline we want to
/// prioritize orders which are more likely to be matchable. This is implemented by rating each
/// order's viability by comparing the ask price with the current market price. The lower the ask
/// price is compared to the market price the higher the chance the order will get prioritized.
fn get_prioritized_orders(
    orders: &[LimitOrder],
    prices: &ExternalPrices,
    order_prioritization_config: &Arguments,
) -> VecDeque<LimitOrder> {
    // Liquidity orders don't make sense on their own and a `SingleOrderSolver` can't
    // settle them together with a user order.
    let mut user_orders: Vec<_> = orders
        .iter()
        .filter(|o| !o.is_liquidity_order)
        .cloned()
        .collect();
    if user_orders.len() <= 1 {
        return user_orders.into();
    }

    let mut rng = rand::thread_rng();

    // Chose `user_orders.len()` distinct items from `user_orders` weighted by the viability of the order.
    // This effectively sorts the orders by viability with a slight randomness to not get stuck on
    // bad orders.
    match user_orders.choose_multiple_weighted(&mut rng, user_orders.len(), |order| {
        let price_viability = estimate_price_viability(order, prices);
        order_prioritization_config.apply_weight_constraints(price_viability)
    }) {
        Ok(weighted_user_orders) => weighted_user_orders.into_iter().cloned().collect(),
        Err(_) => {
            // if weighted sorting by viability fails we fall back to shuffling randomly
            user_orders.shuffle(&mut rng);
            user_orders.into()
        }
    }
}

// Used by the single order solvers to verify that the response respects the order price.
// We have also observed that a 0x buy order did not respect the queried buy amount so verifying
// just the price or verifying just one component of the price (sell amount for buy orders, buy
// amount for sell orders) is not enough.
pub fn execution_respects_order(
    order: &LimitOrder,
    executed_sell_amount: U256,
    executed_buy_amount: U256,
) -> bool {
    // note: This would be different for partially fillable orders but LimitOrder does currently not
    // contain the remaining fill amount.
    executed_sell_amount <= order.sell_amount && executed_buy_amount >= order.buy_amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{liquidity::tests::CapturingSettlementHandler, metrics::NoopMetrics};
    use anyhow::anyhow;
    use maplit::hashmap;
    use model::order::OrderKind;
    use num::{BigRational, FromPrimitive};
    use primitive_types::H160;
    use std::sync::Arc;

    fn test_solver(inner: MockSingleOrderSolving) -> SingleOrderSolver {
        SingleOrderSolver {
            inner: Box::new(inner),
            metrics: Arc::new(NoopMetrics::default()),
            max_merged_settlements: 5,
            max_settlements_per_solver: 5,
            order_prioritization_config: Default::default(),
        }
    }

    #[tokio::test]
    async fn uses_inner_solver() {
        let mut inner = MockSingleOrderSolving::new();
        inner
            .expect_try_settle_order()
            .times(2)
            .returning(|_, _| Ok(Some(Settlement::new(Default::default()))));
        inner.expect_name().returning(|| "Mock Solver");

        let solver = test_solver(inner);
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            settlement_handling: handler.clone(),
            is_liquidity_order: false,
            buy_amount: 1.into(),
            ..Default::default()
        };
        let orders = vec![
            LimitOrder {
                id: 0.into(),
                ..order.clone()
            },
            LimitOrder {
                id: 1.into(),
                ..order.clone()
            },
        ];

        let settlements = solver
            .solve(Auction {
                orders,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(settlements.len(), 3);
    }

    #[tokio::test]
    async fn retries_retryable() {
        let mut inner = MockSingleOrderSolving::new();
        inner.expect_name().return_const("");
        let mut call_count = 0u32;
        inner
            .expect_try_settle_order()
            .times(2)
            .returning(move |_, _| {
                dbg!();
                let result = match call_count {
                    0 => Err(SettlementError {
                        inner: anyhow!(""),
                        retryable: true,
                    }),
                    1 => Ok(None),
                    _ => unreachable!(),
                };
                call_count += 1;
                result
            });

        let solver = test_solver(inner);
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            settlement_handling: handler.clone(),
            ..Default::default()
        };
        solver
            .solve(Auction {
                orders: vec![order],
                ..Default::default()
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn does_not_retry_unretryable() {
        let mut inner = MockSingleOrderSolving::new();
        inner.expect_name().return_const("");
        inner.expect_try_settle_order().times(1).returning(|_, _| {
            Err(SettlementError {
                inner: anyhow!(""),
                retryable: false,
            })
        });

        let solver = test_solver(inner);
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            settlement_handling: handler.clone(),
            ..Default::default()
        };
        solver
            .solve(Auction {
                orders: vec![order],
                ..Default::default()
            })
            .await
            .unwrap();
    }

    #[test]
    fn execution_respects_order_() {
        let order = LimitOrder {
            kind: OrderKind::Sell,
            sell_amount: 10.into(),
            buy_amount: 10.into(),
            ..Default::default()
        };
        assert!(execution_respects_order(&order, 10.into(), 11.into(),));
        assert!(!execution_respects_order(&order, 10.into(), 9.into(),));
        // Unexpectedly the executed sell amount is less than the real sell order for a fill kill
        // order but we still get enough buy token.
        assert!(execution_respects_order(&order, 9.into(), 10.into(),));
        // Price is respected but order is partially filled.
        assert!(!execution_respects_order(&order, 9.into(), 9.into(),));

        let order = LimitOrder {
            kind: OrderKind::Buy,
            ..order
        };
        assert!(execution_respects_order(&order, 9.into(), 10.into(),));
        assert!(!execution_respects_order(&order, 11.into(), 10.into(),));
        // Unexpectedly get more buy amount but sell amount is still respected.
        assert!(execution_respects_order(&order, 10.into(), 11.into(),));
        // Price is respected but order is partially filled.
        assert!(!execution_respects_order(&order, 9.into(), 9.into(),));
    }

    #[ignore] // ignore this test because it could fail due to randomness
    #[test]
    fn spread_orders_get_prioritized() {
        let token = H160::from_low_u64_be;
        let amount = U256::from;
        let order = |sell_amount: u128, is_liquidity_order: bool| LimitOrder {
            sell_token: token(1),
            sell_amount: amount(sell_amount),
            buy_token: token(2),
            buy_amount: amount(100),
            is_liquidity_order,
            ..Default::default()
        };
        let orders = [
            order(500, true),
            order(90, false),
            order(100, false),
            order(130, false),
        ];
        let prices = ExternalPrices::new(
            token(0),
            hashmap! {
                token(1) => BigRational::from_u8(100).unwrap(),
                token(2) => BigRational::from_u8(100).unwrap(),
            },
        )
        .unwrap();

        let config = Arguments::default();

        const SAMPLES: usize = 1_000;
        let mut expected_results = 0;
        for _ in 0..SAMPLES {
            let prioritized_orders = get_prioritized_orders(&orders, &prices, &config);
            let expected_output = &[orders[3].clone(), orders[2].clone(), orders[1].clone()];
            expected_results += usize::from(prioritized_orders == expected_output);
        }
        // Using weighted selection should give us some suboptimal orderings even with skewed
        // weights.
        dbg!(expected_results);
        assert!((expected_results as f64) < (SAMPLES as f64 * 0.9));
    }

    #[ignore] // ignore this test because it could fail due to randomness
    #[test]
    fn tight_orders_get_prioritized() {
        let token = H160::from_low_u64_be;
        let amount = U256::from;
        let order = |sell_amount: u128, is_liquidity_order: bool| LimitOrder {
            sell_token: token(1),
            sell_amount: amount(sell_amount),
            buy_token: token(2),
            buy_amount: amount(100),
            is_liquidity_order,
            ..Default::default()
        };
        let orders = [order(105, false), order(103, false), order(101, false)];
        let prices = ExternalPrices::new(
            token(0),
            hashmap! {
                token(1) => BigRational::from_u8(100).unwrap(),
                token(2) => BigRational::from_u8(100).unwrap(),
            },
        )
        .unwrap();

        let config = Arguments::default();

        const SAMPLES: usize = 1_000;
        let mut expected_results = 0;
        for _ in 0..SAMPLES {
            let prioritized_orders = get_prioritized_orders(&orders, &prices, &config);
            expected_results += usize::from(prioritized_orders == orders);
        }
        // Using weighted selection should give us some suboptimal orderings even with skewed
        // weights.
        dbg!(expected_results);
        assert!((expected_results as f64) < (SAMPLES as f64 * 0.9));
    }
}
