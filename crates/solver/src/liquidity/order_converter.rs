use {
    super::{
        Exchange,
        LimitOrder,
        LimitOrderExecution,
        LimitOrderId,
        LiquidityOrderId,
        SettlementHandling,
    },
    crate::{
        interactions::UnwrapWethInteraction,
        order_balance_filter::BalancedOrder,
        settlement::SettlementEncoder,
    },
    anyhow::{ensure, Context, Result},
    contracts::WETH9,
    ethcontract::U256,
    model::order::{LimitOrderClass, Order, OrderClass, BUY_ETH_ADDRESS},
    std::sync::Arc,
};

pub struct OrderConverter {
    pub native_token: WETH9,
}

impl OrderConverter {
    /// Creates a order converter with the specified WETH9 address for unit
    /// testing purposes.
    #[cfg(test)]
    pub fn test(native_token: ethcontract::H160) -> Self {
        Self {
            native_token: shared::dummy_contract!(WETH9, native_token),
        }
    }

    /// Converts a GPv2 order into a `LimitOrder` type liquidity for solvers.
    ///
    /// The second argument is unused for FOK orders.
    pub fn normalize_limit_order(
        &self,
        BalancedOrder {
            order,
            available_sell_token_balance,
        }: BalancedOrder,
    ) -> Result<LimitOrder> {
        let native_token = self.native_token.clone();
        let buy_token = if order.data.buy_token == BUY_ETH_ADDRESS {
            native_token.address()
        } else {
            order.data.buy_token
        };

        let remaining = shared::remaining_amounts::Remaining::from_order(&order)?;

        let sell_amount = match &order.metadata.class {
            OrderClass::Limit(limit) if !order.data.partially_fillable => {
                compute_synthetic_order_amounts_for_limit_order(&order, limit)?
            }
            _ => order.data.sell_amount,
        };

        let id = match order.metadata.class {
            OrderClass::Market => LimitOrderId::Market(order.metadata.uid),
            OrderClass::Liquidity => {
                LimitOrderId::Liquidity(LiquidityOrderId::Protocol(order.metadata.uid))
            }
            OrderClass::Limit(_) => LimitOrderId::Limit(order.metadata.uid),
        };

        // The reported fee amount that is used for objective computation is the
        // order's full full amount scaled by a constant factor.
        let mut solver_fee = remaining.remaining(order.metadata.solver_fee)?;
        let mut sell_amount = remaining.remaining(sell_amount)?;
        let mut buy_amount = remaining.remaining(order.data.buy_amount)?;

        // Partially fillable orders are included in the auction when there is at least
        // 1 atom balance available.
        if order.data.partially_fillable {
            let need = sell_amount
                .checked_add(remaining.remaining(order.data.fee_amount)?)
                .context("partially fillable need calculation overflow")?;
            let have = available_sell_token_balance;
            anyhow::ensure!(
                have != 0.into(),
                "unexpected 0 balance for partially fillable order"
            );
            tracing::trace!(%need, %have, "partially fillable order conversion");
            if have < need {
                solver_fee = solver_fee
                    .checked_mul(have)
                    .context("partially fillable solver_fee calculation overflow")?
                    .checked_div(need)
                    .context("partially fillable solver_fee calculation overflow")?;
                sell_amount = sell_amount
                    .checked_mul(have)
                    .context("partially fillable sell_amount calculation overflow")?
                    .checked_div(need)
                    .context("partially fillable sell_amount calculation overflow")?;
                buy_amount = buy_amount
                    .checked_mul(have)
                    .context("partially fillable buy_amount calculation overflow")?
                    .checked_div(need)
                    .context("partially fillable buy_amount calculation overflow")?;
            }
        }

        ensure!(
            !sell_amount.is_zero() && !buy_amount.is_zero(),
            "partially fillable order scaled to 0 amounts",
        );

        Ok(LimitOrder {
            id,
            sell_token: order.data.sell_token,
            buy_token,
            sell_amount,
            buy_amount,
            kind: order.data.kind,
            partially_fillable: order.data.partially_fillable,
            solver_fee,
            settlement_handling: Arc::new(OrderSettlementHandler {
                order,
                native_token,
            }),
            exchange: Exchange::GnosisProtocol,
        })
    }
}

struct OrderSettlementHandler {
    order: Order,
    native_token: WETH9,
}

/// Returns the `sell_amount` adjusted for limit orders.
fn compute_synthetic_order_amounts_for_limit_order(
    order: &Order,
    limit: &LimitOrderClass,
) -> Result<U256> {
    anyhow::ensure!(
        order.metadata.class.is_limit(),
        "this function should only be called for limit orders"
    );
    // Solvable limit orders always have a surplus fee. It would be nice if this was
    // enforced in the API.
    let surplus_fee = limit
        .surplus_fee
        .context("solvable order without surplus fee")?;

    order
        .data
        .sell_amount
        .checked_add(order.data.fee_amount)
        .context("surplus_fee adjustment would overflow sell_amount")?
        .checked_sub(surplus_fee)
        .context("surplus_fee adjustment would underflow sell_amount")
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(
        &self,
        execution: LimitOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> Result<()> {
        let is_native_token_buy_order = self.order.data.buy_token == BUY_ETH_ADDRESS;
        if is_native_token_buy_order {
            encoder.add_token_equivalency(self.native_token.address(), BUY_ETH_ADDRESS)?;
        }

        let trade =
            encoder.add_trade(self.order.clone(), execution.filled, execution.solver_fee)?;

        if is_native_token_buy_order {
            encoder.add_unwrap(UnwrapWethInteraction {
                weth: self.native_token.clone(),
                amount: trade.buy_amount,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::settlement::tests::assert_settlement_encoded_with,
        ethcontract::H160,
        maplit::hashmap,
        model::order::{OrderBuilder, OrderData, OrderKind, OrderMetadata},
        shared::dummy_contract,
    };

    #[test]
    fn eth_buy_liquidity_is_assigned_to_weth() {
        let native_token = H160([0x42; 20]);
        let converter = OrderConverter::test(native_token);
        let order = Order {
            data: OrderData {
                buy_token: BUY_ETH_ADDRESS,
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            converter
                .normalize_limit_order(BalancedOrder::full(order))
                .unwrap()
                .buy_token,
            native_token,
        );
    }

    #[test]
    fn non_eth_buy_liquidity_stays_put() {
        let buy_token = H160([0x21; 20]);
        let converter = OrderConverter::test(H160([0x42; 20]));
        let order = Order {
            data: OrderData {
                buy_token,
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            converter
                .normalize_limit_order(BalancedOrder::full(order))
                .unwrap()
                .buy_token,
            buy_token
        );
    }

    #[test]
    fn adds_unwrap_interaction_for_sell_order_with_eth_flag() {
        let native_token_address = H160([0x42; 20]);
        let sell_token = H160([0x21; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);

        let execution = LimitOrderExecution::new(1337.into(), 0.into());
        let executed_buy_amount = U256::from(2 * 1337);
        let solver_fee = U256::from(1234);

        let prices = hashmap! {
            native_token.address() => U256::from(100),
            sell_token => U256::from(200),
        };
        let order = Order {
            data: OrderData {
                buy_token: BUY_ETH_ADDRESS,
                sell_token,
                sell_amount: 1337.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };
        println!("{}", order.data.buy_token);

        let order_settlement_handler = OrderSettlementHandler {
            order: order.clone(),
            native_token: native_token.clone(),
        };

        assert_settlement_encoded_with(
            prices,
            order_settlement_handler,
            execution.clone(),
            |encoder| {
                encoder
                    .add_token_equivalency(native_token.address(), BUY_ETH_ADDRESS)
                    .unwrap();
                encoder.add_unwrap(UnwrapWethInteraction {
                    weth: native_token,
                    amount: executed_buy_amount,
                });
                assert!(encoder
                    .add_trade(order, execution.filled, solver_fee)
                    .is_ok());
            },
        );
    }

    #[test]
    fn adds_unwrap_interaction_for_buy_order_with_eth_flag() {
        for class in [
            OrderClass::Market,
            OrderClass::Limit(LimitOrderClass {
                surplus_fee: Some(Default::default()),
                surplus_fee_timestamp: Some(Default::default()),
                executed_surplus_fee: None,
            }),
            OrderClass::Liquidity,
        ] {
            let native_token_address = H160([0x42; 20]);
            let sell_token = H160([0x21; 20]);
            let native_token = dummy_contract!(WETH9, native_token_address);
            let execution = LimitOrderExecution::new(1337.into(), 0.into());
            let prices = hashmap! {
                native_token.address() => U256::from(1),
                sell_token => U256::from(2),
            };
            let order = Order {
                data: OrderData {
                    buy_token: BUY_ETH_ADDRESS,
                    buy_amount: 1337.into(),
                    sell_token,
                    kind: OrderKind::Buy,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    class,
                    ..Default::default()
                },
                ..Default::default()
            };
            println!("{}", order.data.buy_token);

            let order_settlement_handler = OrderSettlementHandler {
                order: order.clone(),
                native_token: native_token.clone(),
            };

            assert_settlement_encoded_with(
                prices,
                order_settlement_handler,
                execution.clone(),
                |encoder| {
                    encoder
                        .add_token_equivalency(native_token.address(), BUY_ETH_ADDRESS)
                        .unwrap();
                    assert!(encoder.add_trade(order, execution.filled, 0.into()).is_ok());
                    encoder.add_unwrap(UnwrapWethInteraction {
                        weth: native_token,
                        amount: execution.filled,
                    });
                },
            );
        }
    }

    #[test]
    fn does_not_add_unwrap_interaction_for_order_without_eth_flag() {
        let native_token_address = H160([0x42; 20]);
        let sell_token = H160([0x21; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);
        let not_buy_eth_address = H160([0xff; 20]);
        assert_ne!(not_buy_eth_address, BUY_ETH_ADDRESS);

        let execution = LimitOrderExecution::new(1337.into(), 0.into());
        let prices = hashmap! {
            not_buy_eth_address => U256::from(100),
            sell_token => U256::from(200),
        };
        let order = Order {
            data: OrderData {
                buy_token: not_buy_eth_address,
                buy_amount: 1337.into(),
                sell_token,
                sell_amount: 1337.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let order_settlement_handler = OrderSettlementHandler {
            order: order.clone(),
            native_token,
        };

        assert_settlement_encoded_with(
            prices,
            order_settlement_handler,
            execution.clone(),
            |encoder| {
                assert!(encoder.add_trade(order, execution.filled, 0.into()).is_ok());
            },
        );
    }

    #[test]
    fn scales_limit_order_amounts_for_partially_filled_orders() {
        let converter = OrderConverter::test(H160::default());
        let mut order = Order {
            data: OrderData {
                sell_amount: 20.into(),
                buy_amount: 40.into(),
                fee_amount: 60.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: 10.into(),
                solver_fee: 200.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let order_ = converter
            .normalize_limit_order(BalancedOrder {
                order: order.clone(),
                available_sell_token_balance: 1000.into(),
            })
            .unwrap();
        // Amounts are halved because the order is half executed.
        assert_eq!(order_.sell_amount, 10.into());
        assert_eq!(order_.buy_amount, 20.into());
        assert_eq!(order_.solver_fee, 100.into());

        let order_ = converter
            .normalize_limit_order(BalancedOrder {
                order: order.clone(),
                available_sell_token_balance: 20.into(),
            })
            .unwrap();
        // Amounts are quartered because of balance.
        assert_eq!(order_.sell_amount, 5.into());
        assert_eq!(order_.buy_amount, 10.into());
        assert_eq!(order_.solver_fee, 50.into());

        order.metadata.executed_sell_amount_before_fees = 0.into();
        let order_ = converter
            .normalize_limit_order(BalancedOrder {
                order,
                available_sell_token_balance: 20.into(),
            })
            .unwrap();
        // Amounts are still quartered because of balance.
        assert_eq!(order_.sell_amount, 5.into());
        assert_eq!(order_.buy_amount, 10.into());
        assert_eq!(order_.solver_fee, 50.into());
    }

    #[test]
    fn limit_orders_get_adjusted_for_surplus_fee() {
        let converter = OrderConverter::test(Default::default());
        let order = OrderBuilder::default()
            .with_class(OrderClass::Limit(Default::default()))
            .with_sell_amount(1_000.into())
            .with_buy_amount(1.into())
            .with_fee_amount(200.into())
            .with_surplus_fee(100.into())
            .with_solver_fee(200.into())
            .build();
        let solver_order = converter
            .normalize_limit_order(BalancedOrder::full(order))
            .unwrap();

        // sell_amount + fee_amount - surplus_fee = 1_000 + 200 - 100
        assert_eq!(solver_order.sell_amount, 1_100.into());
        // it's the `autopilot`'s responsibility to prepare this value for us so we
        // don't touch it
        assert_eq!(solver_order.solver_fee, 200.into());
    }

    #[test]
    fn limit_orders_scaled_to_zero_amounts_rejected() {
        let converter = OrderConverter::test(Default::default());

        let sell = Order {
            data: OrderData {
                sell_amount: 100.into(),
                buy_amount: 10.into(),
                fee_amount: 0.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut sell = BalancedOrder {
            order: sell,
            available_sell_token_balance: 100.into(),
        };

        assert!(converter.normalize_limit_order(sell.clone()).is_ok());

        // Execute the order so that scaling the buy_amount would result in a
        // 0 amount.
        sell.order.metadata.executed_sell_amount = 99_u32.into();
        sell.order.metadata.executed_sell_amount_before_fees = 99_u32.into();
        sell.order.metadata.executed_buy_amount = 10_u32.into();

        assert!(converter.normalize_limit_order(sell).is_err());

        let buy = Order {
            data: OrderData {
                sell_amount: 10.into(),
                buy_amount: 100.into(),
                fee_amount: 0.into(),
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buy = BalancedOrder {
            order: buy,
            available_sell_token_balance: 10.into(),
        };

        assert!(converter.normalize_limit_order(buy.clone()).is_ok());

        // Execute the order so that scaling the sell_amount would result in a
        // 0 amount.
        buy.order.metadata.executed_sell_amount = 10_u32.into();
        buy.order.metadata.executed_sell_amount_before_fees = 10_u32.into();
        buy.order.metadata.executed_buy_amount = 99_u32.into();

        assert!(converter.normalize_limit_order(buy).is_err());
    }
}
