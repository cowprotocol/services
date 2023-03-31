use {
    crate::{
        liquidity::{LimitOrder, LimitOrderExecution, LimitOrderId},
        metrics::SolverMetrics,
        settlement::Settlement,
        solver::{Auction, Solver},
    },
    anyhow::{Context as _, Error, Result},
    clap::Parser,
    ethcontract::Account,
    model::order::OrderKind,
    num::ToPrimitive,
    number_conversions::u256_to_big_rational,
    primitive_types::U256,
    rand::prelude::SliceRandom,
    shared::{
        conversions::U256Ext,
        external_prices::ExternalPrices,
        interaction::Interaction,
        price_estimation::gas,
    },
    std::{
        collections::VecDeque,
        fmt::{self, Display, Formatter},
        sync::Arc,
        time::Duration,
    },
    tracing::Instrument,
};

mod fills;
mod merge;

/// CLI arguments to configure order prioritization of single order solvers
/// based on an orders price.
#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Arguments {
    /// Exponent to turn an order's price ratio into a weight for a weighted
    /// prioritization.
    #[clap(long, env, default_value = "10.0")]
    pub price_priority_exponent: f64,

    /// The lowest possible weight an order can have for the weighted order
    /// prioritization.
    #[clap(long, env, default_value = "0.01")]
    pub price_priority_min_weight: f64,

    /// The highest possible weight an order can have for the weighted order
    /// prioritization.
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
        original_weight.powf(self.price_priority_exponent).clamp(
            self.price_priority_min_weight,
            self.price_priority_max_weight,
        )
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
/// Implementations of this trait know how to settle a single limit order (not
/// taking advantage of batching multiple orders together)
pub trait SingleOrderSolving: Send + Sync + 'static {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        auction: &Auction,
    ) -> Result<Option<SingleOrderSettlement>, SettlementError>;

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
    fills: fills::Fills,
}

impl SingleOrderSolver {
    pub fn new(
        inner: Box<dyn SingleOrderSolving>,
        metrics: Arc<dyn SolverMetrics>,
        max_settlements_per_solver: usize,
        max_merged_settlements: usize,
        order_prioritization_config: Arguments,
        smallest_fill: U256,
    ) -> Self {
        Self {
            inner,
            metrics,
            max_merged_settlements,
            max_settlements_per_solver,
            order_prioritization_config,
            fills: fills::Fills::new(smallest_fill),
        }
    }

    async fn solve_single_order(&self, order: LimitOrder, auction: &Auction) -> SolveResult {
        let name = self.inner.name();
        let Some(fill) = self.fills.order(&order, &auction.external_prices) else {
            tracing::warn!(?order.id, "failed to compute fill; skipping order");
            return SolveResult::Failed;
        };

        let single = match self.inner.try_settle_order(fill.clone(), auction).await {
            Ok(value) => {
                self.metrics.single_order_solver_succeeded(name);
                value
            }
            Err(err) => {
                self.metrics.single_order_solver_failed(name);
                if err.retryable {
                    tracing::warn!("Solver {} retryable error: {:?}", name, &err.inner);
                    return SolveResult::Retryable(order);
                } else {
                    tracing::warn!("Solver {} error: {:?}", name, &err.inner);
                    return SolveResult::Failed;
                }
            }
        };

        match single.and_then(|single| {
            single
                .into_settlement(
                    &order,
                    fill.full_execution_amount(),
                    &auction.external_prices,
                    auction.gas_price,
                )
                .transpose()
        }) {
            Some(Ok(settlement)) => {
                if let Some(order_uid) = order.id.order_uid() {
                    // Maybe some liquidity appeared that enables a bigger fill.
                    self.fills.increase_next_try(order_uid);
                }
                SolveResult::Solved(settlement)
            }
            Some(Err(err)) => {
                tracing::warn!(%name, ?err, "encoding error");
                SolveResult::Failed
            }
            None => {
                if let Some(order_uid) = order.id.order_uid() {
                    self.fills.reduce_next_try(order_uid);
                }
                SolveResult::Failed
            }
        }
    }
}

enum SolveResult {
    /// No solution but order could be retried.
    Retryable(LimitOrder),
    /// No solution and retrying would not help.
    Failed,
    /// Found a solution for the order.
    Solved(Settlement),
}

#[async_trait::async_trait]
impl Solver for SingleOrderSolver {
    async fn solve(&self, auction: Auction) -> Result<Vec<Settlement>> {
        let mut orders = get_prioritized_orders(
            &auction.orders,
            &auction.external_prices,
            &self.order_prioritization_config,
        );
        tracing::trace!(name = self.name(), ?orders, "prioritized orders");

        let mut settlements = Vec::new();
        let settle = async {
            while let Some(order) = orders.pop_front() {
                let span = tracing::info_span!("solve", id =? order.id, solver = self.name());
                match self
                    .solve_single_order(order, &auction)
                    .instrument(span)
                    .await
                {
                    SolveResult::Failed => continue,
                    SolveResult::Retryable(order) => orders.push_back(order),
                    SolveResult::Solved(settlement) => settlements.push(settlement),
                }
            }
        };

        // Subtract a small amount of time to ensure that the driver doesn't reach the
        // deadline first.
        let _ = tokio::time::timeout_at(
            auction
                .deadline
                .checked_sub(Duration::from_secs(1))
                .unwrap()
                .into(),
            settle,
        )
        .await;

        self.fills.collect_garbage();

        // Keep at most this many settlements. This is important in case where a solver
        // produces a large number of settlements which would hold up the driver
        // logic when simulating them.
        // Shuffle first so that in the case a buggy solver keeps returning some amount
        // of invalid settlements first we have a chance to make progress.
        settlements.shuffle(&mut rand::thread_rng());
        settlements.truncate(self.max_settlements_per_solver);

        merge::merge_settlements(
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

#[derive(Debug, Default)]
pub struct SingleOrderSettlement {
    pub sell_token_price: U256,
    pub buy_token_price: U256,
    pub interactions: Vec<Box<dyn Interaction>>,
    pub gas_estimate: U256,
}

impl SingleOrderSettlement {
    pub fn into_settlement(
        self,
        order: &LimitOrder,
        executed_amount: U256,
        prices: &ExternalPrices,
        gas_price: f64,
    ) -> Result<Option<Settlement>> {
        // Compute the expected traded amounts.
        let (traded_sell_amount, traded_buy_amount) = match order.kind {
            OrderKind::Buy => (
                executed_amount
                    .checked_mul(self.buy_token_price)
                    .context("buy value overflow")?
                    .checked_div(self.sell_token_price)
                    .context("zero sell token price")?,
                executed_amount,
            ),
            OrderKind::Sell => (
                executed_amount,
                executed_amount
                    .checked_mul(self.sell_token_price)
                    .context("sell value overflow")?
                    .checked_ceil_div(&self.buy_token_price)
                    .context("zero buy token price")?,
            ),
        };

        // Compute the surplus fee that needs to be incorporated into the prices
        // and solver fee which will be used for scoring.
        let (surplus_fee, solver_fee) = if order.solver_determines_fee() {
            let gas_estimate = self.gas_estimate + gas::SETTLEMENT_OVERHEAD;
            let gas_cost = gas_estimate * U256::from_f64_lossy(gas_price);

            let fee = number_conversions::big_rational_to_u256(
                &prices
                    .try_get_token_amount(
                        &number_conversions::u256_to_big_rational(&gas_cost),
                        order.sell_token,
                    )
                    .context("failed to compute solver fee")?,
            )
            .context("invalid solver fee amount")?;

            (fee, fee)
        } else {
            (U256::zero(), order.solver_fee)
        };

        // Compute the actual executed amounts accounting for surplus fees.
        let (executed_sell_amount, executed_buy_amount) = match order.kind {
            OrderKind::Buy => (
                traded_sell_amount
                    .checked_add(surplus_fee)
                    .context("underflow computing executed sell amount")?,
                traded_buy_amount,
            ),
            OrderKind::Sell => {
                let executed_sell_amount = traded_sell_amount
                    .checked_add(surplus_fee)
                    .context("overflow computing executed sell amount")?
                    .min(order.sell_amount);
                let executed_buy_amount = executed_sell_amount
                    .checked_sub(surplus_fee)
                    .context("underflow computing executed buy amount")?
                    .checked_mul(traded_buy_amount)
                    .context("overflow computing executed buy amount")?
                    .checked_ceil_div(&traded_sell_amount)
                    .context("zero traded sell amount")?;

                (executed_sell_amount, executed_buy_amount)
            }
        };

        // Check that the order's limit price is satisfied accounting for the
        // surplus fees.
        if order
            .sell_amount
            .checked_mul(executed_buy_amount)
            .context("overflow sell value")?
            < order
                .buy_amount
                .checked_mul(executed_sell_amount)
                .context("overflow buy value")?
        {
            tracing::debug!(
                ?surplus_fee,
                ?self.sell_token_price,
                ?self.buy_token_price,
                ?order,
                "order limit price not respected after applying surplus fees",
            );
            return Ok(None);
        }

        let prices = [
            (order.sell_token, executed_buy_amount),
            (order.buy_token, executed_sell_amount - surplus_fee),
        ];
        let mut settlement = Settlement::new(prices.into_iter().collect());
        let execution = LimitOrderExecution {
            filled: match order.kind {
                OrderKind::Buy => executed_buy_amount,
                OrderKind::Sell => executed_sell_amount - surplus_fee,
            },
            solver_fee,
        };
        settlement.with_liquidity(order, execution)?;
        for interaction in self.interactions {
            settlement.encoder.append_to_execution_plan(interaction);
        }
        Ok(Some(settlement))
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

/// Prioritizes orders to settle in the auction. First come all the market
/// orders and then all the limit orders. Orders within these groups get
/// prioritized by their price achievability. See [`prioritize_orders`].
fn get_prioritized_orders(
    orders: &[LimitOrder],
    prices: &ExternalPrices,
    order_prioritization_config: &Arguments,
) -> VecDeque<LimitOrder> {
    let (market, limit) = orders
        .iter()
        .filter(|order| !matches!(order.id, LimitOrderId::Liquidity(_)))
        .cloned()
        .partition(|order| matches!(order.id, LimitOrderId::Market(_)));

    let market = prioritize_orders(market, prices, order_prioritization_config);
    let limit = prioritize_orders(limit, prices, order_prioritization_config);

    market.into_iter().chain(limit.into_iter()).collect()
}

/// Returns the `native_sell_amount / native_buy_amount` of the given order
/// under the current market conditions. The higher the value the more likely it
/// is that this order could get filled.
fn estimate_price_viability(order: &LimitOrder, prices: &ExternalPrices) -> f64 {
    let sell_amount = u256_to_big_rational(&order.sell_amount);
    let buy_amount = u256_to_big_rational(&order.buy_amount);
    let native_sell_amount = prices.get_native_amount(order.sell_token, sell_amount);
    let native_buy_amount = prices.get_native_amount(order.buy_token, buy_amount);
    (native_sell_amount / native_buy_amount)
        .to_f64()
        .unwrap_or(0.)
}

/// In case there are too many orders to solve before the auction deadline we
/// want to prioritize orders which are more likely to be matchable. This is
/// implemented by rating each order's viability by comparing the ask price with
/// the current market price. The lower the ask price is compared to the market
/// price the higher the chance the order will get prioritized.
fn prioritize_orders(
    mut orders: Vec<LimitOrder>,
    prices: &ExternalPrices,
    order_prioritization_config: &Arguments,
) -> Vec<LimitOrder> {
    if orders.len() <= 1 {
        return orders;
    }

    let mut rng = rand::thread_rng();

    // Chose `user_orders.len()` distinct items from `user_orders` weighted by the
    // viability of the order. This effectively sorts the orders by viability
    // with a slight randomness to not get stuck on bad orders.
    match orders.choose_multiple_weighted(&mut rng, orders.len(), |order| {
        let price_viability = estimate_price_viability(order, prices);
        order_prioritization_config.apply_weight_constraints(price_viability)
    }) {
        Ok(weighted_user_orders) => weighted_user_orders.into_iter().cloned().collect(),
        Err(err) => {
            // if weighted sorting by viability fails we fall back to shuffling randomly
            tracing::warn!(?err, "weighted order prioritization failed");
            orders.shuffle(&mut rng);
            orders
        }
    }
}

// Used by the single order solvers to verify that the response respects the
// order price. We have also observed that a 0x buy order did not respect the
// queried buy amount so verifying just the price or verifying just one
// component of the price (sell amount for buy orders, buy amount for sell
// orders) is not enough.
pub fn execution_respects_order(
    order: &LimitOrder,
    executed_sell_amount: U256,
    executed_buy_amount: U256,
) -> bool {
    // note: This would be different for partially fillable orders but LimitOrder
    // does currently not contain the remaining fill amount.
    executed_sell_amount <= order.sell_amount && executed_buy_amount >= order.buy_amount
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            liquidity::{
                order_converter::OrderConverter,
                tests::CapturingSettlementHandler,
                LimitOrderId,
                LiquidityOrderId,
            },
            metrics::NoopMetrics,
            settlement::TradeExecution,
        },
        anyhow::anyhow,
        ethcontract::Bytes,
        maplit::{btreemap, hashmap},
        model::order::{Order, OrderClass, OrderData, OrderKind, OrderMetadata, OrderUid},
        num::{BigRational, FromPrimitive},
        primitive_types::H160,
        shared::http_solver::model::InternalizationStrategy,
        std::sync::Arc,
    };

    fn test_solver(inner: MockSingleOrderSolving) -> SingleOrderSolver {
        SingleOrderSolver {
            inner: Box::new(inner),
            metrics: Arc::new(NoopMetrics::default()),
            max_merged_settlements: 5,
            max_settlements_per_solver: 5,
            order_prioritization_config: Default::default(),
            fills: fills::Fills::new(1.into()),
        }
    }

    #[tokio::test]
    async fn merges() {
        let native = H160::from_low_u64_be(0);
        let converter = OrderConverter::test(native);
        let buy_order = Order {
            data: OrderData {
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                kind: OrderKind::Buy,
                sell_amount: 2.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid: OrderUid([0u8; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        let sell_order = Order {
            data: OrderData {
                sell_token: H160::from_low_u64_be(3),
                buy_token: H160::from_low_u64_be(4),
                sell_amount: 7.into(),
                buy_amount: 6.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid: OrderUid([1u8; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        let orders = [&buy_order, &sell_order]
            .iter()
            .map(|order| {
                converter
                    .normalize_limit_order(Order::clone(order))
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let mut inner = MockSingleOrderSolving::new();
        inner
            .expect_try_settle_order()
            .returning(|order, _| match order.kind {
                OrderKind::Buy => Ok(Some(SingleOrderSettlement {
                    sell_token_price: 1.into(),
                    buy_token_price: 2.into(),
                    interactions: vec![Box::new((
                        H160::from_low_u64_be(3),
                        4.into(),
                        Bytes(vec![5]),
                    ))],
                    gas_estimate: U256::zero(),
                })),
                OrderKind::Sell => Ok(Some(SingleOrderSettlement {
                    sell_token_price: 6.into(),
                    buy_token_price: 7.into(),
                    interactions: vec![Box::new((
                        H160::from_low_u64_be(8),
                        9.into(),
                        Bytes(vec![10]),
                    ))],
                    gas_estimate: U256::zero(),
                })),
            });
        inner.expect_name().returning(|| "");

        let solver = test_solver(inner);
        let external_prices = ExternalPrices::try_from_auction_prices(
            native,
            [
                buy_order.data.sell_token,
                buy_order.data.buy_token,
                sell_order.data.sell_token,
                sell_order.data.buy_token,
            ]
            .into_iter()
            .map(|token| (token, U256::from(1)))
            .collect(),
        )
        .unwrap();
        let settlements = solver
            .solve(Auction {
                external_prices,
                orders,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(settlements.len(), 3);

        let merged = settlements.into_iter().nth(2).unwrap().encoder;
        let merged = merged.finish(InternalizationStrategy::EncodeAllInteractions);
        assert_eq!(merged.tokens.len(), 4);
        let token_index = |token: &H160| -> usize {
            merged
                .tokens
                .iter()
                .position(|token_| token_ == token)
                .unwrap()
        };
        let prices = &merged.clearing_prices;
        assert_eq!(prices[token_index(&buy_order.data.sell_token)], 1.into());
        assert_eq!(prices[token_index(&buy_order.data.buy_token)], 2.into());
        assert_eq!(prices[token_index(&sell_order.data.sell_token)], 6.into());
        assert_eq!(prices[token_index(&sell_order.data.buy_token)], 7.into());
        assert_eq!(merged.trades.len(), 2);
        assert_eq!(merged.interactions.iter().flatten().count(), 2);
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
        // Unexpectedly the executed sell amount is less than the real sell order for a
        // fill kill order but we still get enough buy token.
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
        let order = |sell_amount: u128, id: LimitOrderId| LimitOrder {
            id,
            sell_token: token(1),
            sell_amount: amount(sell_amount),
            buy_token: token(2),
            buy_amount: amount(100),
            ..Default::default()
        };
        let orders = vec![
            order(
                500,
                LimitOrderId::Liquidity(LiquidityOrderId::Protocol(OrderUid::from_integer(1))),
            ), //liquidity order
            order(90, Default::default()),  //market order
            order(100, Default::default()), //market order
            order(130, Default::default()), //market order
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
            let prioritized_orders = prioritize_orders(orders.clone(), &prices, &config);
            let expected_output = &[orders[3].clone(), orders[2].clone(), orders[1].clone()];
            expected_results += usize::from(prioritized_orders == expected_output);
        }
        // Using weighted selection should give us some suboptimal orderings even with
        // skewed weights.
        dbg!(expected_results);
        assert!((expected_results as f64) < (SAMPLES as f64 * 0.9));
    }

    #[ignore] // ignore this test because it could fail due to randomness
    #[test]
    fn tight_orders_get_prioritized() {
        let token = H160::from_low_u64_be;
        let amount = U256::from;
        let order = |sell_amount: u128, id: LimitOrderId| LimitOrder {
            id,
            sell_token: token(1),
            sell_amount: amount(sell_amount),
            buy_token: token(2),
            buy_amount: amount(100),
            ..Default::default()
        };
        let orders = vec![
            order(105, Default::default()), //market order
            order(103, Default::default()), //market order
            order(101, Default::default()), //market order
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
            let prioritized_orders = prioritize_orders(orders.clone(), &prices, &config);
            expected_results += usize::from(prioritized_orders == orders);
        }
        // Using weighted selection should give us some suboptimal orderings even with
        // skewed weights.
        dbg!(expected_results);
        assert!((expected_results as f64) < (SAMPLES as f64 * 0.9));
    }

    #[test]
    fn partially_fillable_single_order_settlements() {
        let native = H160::from_low_u64_be(0);
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let converter = OrderConverter::test(native);
        let order = |kind: OrderKind| {
            converter
                .normalize_limit_order(Order {
                    data: OrderData {
                        sell_token: H160::from_low_u64_be(1),
                        buy_token: H160::from_low_u64_be(2),
                        kind,
                        sell_amount: 100.into(),
                        buy_amount: 100.into(),
                        partially_fillable: true,
                        ..Default::default()
                    },
                    metadata: OrderMetadata {
                        uid: OrderUid([0u8; 56]),
                        class: OrderClass::Limit(Default::default()),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .unwrap()
        };

        let base = U256::from(10_u128.pow(18));
        let prices = ExternalPrices::try_from_auction_prices(
            native,
            btreemap! {
                native => base,
                sell_token => base * 50000,
                buy_token => base * 50000,
            },
        )
        .unwrap();

        let settlement = |order: LimitOrder, in_amount: u128, out_amount: u128| {
            SingleOrderSettlement {
                sell_token_price: out_amount.into(),
                buy_token_price: in_amount.into(),
                interactions: vec![],
                gas_estimate: 2.into(),
            }
            .into_settlement(
                &order,
                match order.kind {
                    OrderKind::Buy => out_amount.into(),
                    OrderKind::Sell => in_amount.into(),
                },
                &prices,
                1.,
            )
            .unwrap()
        };
        let trade = |settlement: Settlement| settlement.trade_executions().next().unwrap();

        // Not enough room for surplus fees.
        assert!(settlement(order(OrderKind::Buy), 50, 50).is_none());
        assert!(settlement(order(OrderKind::Buy), 100, 100).is_none());
        assert!(settlement(order(OrderKind::Sell), 50, 50).is_none());
        assert!(settlement(order(OrderKind::Sell), 100, 100).is_none());

        // Adds surplus fee to executed sell amount.
        assert_eq!(
            trade(settlement(order(OrderKind::Buy), 40, 50).unwrap()),
            TradeExecution {
                sell_token,
                buy_token,
                sell_amount: 42.into(),
                buy_amount: 50.into(),
                fee_amount: 2.into(),
            }
        );
        assert_eq!(
            trade(settlement(order(OrderKind::Buy), 90, 100).unwrap()),
            TradeExecution {
                sell_token,
                buy_token,
                sell_amount: 92.into(),
                buy_amount: 100.into(),
                fee_amount: 2.into(),
            }
        );
        assert_eq!(
            trade(settlement(order(OrderKind::Sell), 50, 60).unwrap()),
            TradeExecution {
                sell_token,
                buy_token,
                sell_amount: 52.into(),
                buy_amount: 60.into(),
                fee_amount: 2.into(),
            }
        );

        // Scale buy amount if insufficient "space" for collecting surplus fees
        // in the sell token.
        assert_eq!(
            trade(settlement(order(OrderKind::Sell), 99, 110).unwrap()),
            TradeExecution {
                sell_token,
                buy_token,
                sell_amount: 100.into(),
                buy_amount: 109.into(),
                fee_amount: 2.into(),
            }
        );
        assert_eq!(
            trade(settlement(order(OrderKind::Sell), 100, 110).unwrap()),
            TradeExecution {
                sell_token,
                buy_token,
                sell_amount: 100.into(),
                buy_amount: 108.into(),
                fee_amount: 2.into(),
            }
        );
    }
}
