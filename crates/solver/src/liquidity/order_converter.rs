use {
    super::{
        BalancedOrder,
        Exchange,
        LimitOrder,
        LimitOrderExecution,
        LimitOrderId,
        LiquidityOrderId,
        SettlementHandling,
    },
    crate::{interactions::UnwrapWethInteraction, settlement::SettlementEncoder},
    anyhow::{ensure, Result},
    contracts::WETH9,
    model::order::{Order, OrderClass, BUY_ETH_ADDRESS},
    std::sync::Arc,
};

#[derive(Clone)]
pub struct OrderConverter {
    pub native_token: WETH9,
}

impl OrderConverter {
    /// Creates a order converter with the specified WETH9 address for unit
    /// testing purposes.
    #[cfg(test)]
    pub fn test(native_token: ethcontract::H160) -> Self {
        Self {
            native_token: contracts::dummy_contract!(WETH9, native_token),
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
        allow_unwrap_native_token: bool,
    ) -> Result<LimitOrder> {
        let buy_token = if order.data.buy_token == BUY_ETH_ADDRESS {
            self.native_token.address()
        } else {
            order.data.buy_token
        };

        let remaining = shared::remaining_amounts::Remaining::from_order_with_balance(
            &(&order).into(),
            available_sell_token_balance,
        )?;

        let sell_amount = order.data.sell_amount;

        let id = match order.metadata.class {
            OrderClass::Market => LimitOrderId::Market(order.metadata.uid),
            OrderClass::Liquidity => {
                LimitOrderId::Liquidity(LiquidityOrderId::Protocol(order.metadata.uid))
            }
            OrderClass::Limit => LimitOrderId::Limit(order.metadata.uid),
        };
        let sell_amount = remaining.remaining(sell_amount)?;
        let buy_amount = remaining.remaining(order.data.buy_amount)?;
        ensure!(
            !sell_amount.is_zero() && !buy_amount.is_zero(),
            "order with 0 amounts",
        );

        Ok(LimitOrder {
            id,
            sell_token: order.data.sell_token,
            buy_token,
            sell_amount,
            buy_amount,
            kind: order.data.kind,
            partially_fillable: order.data.partially_fillable,
            user_fee: remaining.remaining(order.data.fee_amount)?,
            settlement_handling: Arc::new(OrderSettlementHandler {
                order,
                native_token: self.native_token.clone(),
                allow_unwrap_native_token,
            }),
            exchange: Exchange::GnosisProtocol,
        })
    }
}

struct OrderSettlementHandler {
    order: Order,
    native_token: WETH9,
    allow_unwrap_native_token: bool,
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
        let allow_unwrap_native_token =
            self.order.data.buy_token == BUY_ETH_ADDRESS && self.allow_unwrap_native_token;
        if allow_unwrap_native_token {
            encoder.add_token_equivalency(self.native_token.address(), BUY_ETH_ADDRESS)?;
        }

        let trade = encoder.add_trade(self.order.clone(), execution.filled, execution.fee)?;

        if allow_unwrap_native_token {
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
        contracts::dummy_contract,
        ethcontract::H160,
        maplit::hashmap,
        model::order::{OrderData, OrderKind, OrderMetadata},
        primitive_types::U256,
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
                .normalize_limit_order(BalancedOrder::full(order), true)
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
                .normalize_limit_order(BalancedOrder::full(order), true)
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
        let fee = U256::from(1234);

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
            allow_unwrap_native_token: true,
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
                assert!(encoder.add_trade(order, execution.filled, fee).is_ok());
            },
        );
    }

    #[test]
    fn adds_unwrap_interaction_for_buy_order_with_eth_flag() {
        for class in [OrderClass::Market, OrderClass::Limit, OrderClass::Liquidity] {
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
                allow_unwrap_native_token: true,
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
            allow_unwrap_native_token: true,
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
                solver_fee: 60.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let order_ = converter
            .normalize_limit_order(
                BalancedOrder {
                    order: order.clone(),
                    available_sell_token_balance: 1000.into(),
                },
                true,
            )
            .unwrap();
        // Amounts are halved because the order is half executed.
        assert_eq!(order_.sell_amount, 10.into());
        assert_eq!(order_.buy_amount, 20.into());
        assert_eq!(order_.user_fee, 30.into());

        let order_ = converter
            .normalize_limit_order(
                BalancedOrder {
                    order: order.clone(),
                    available_sell_token_balance: 20.into(),
                },
                true,
            )
            .unwrap();
        // Amounts are quartered because of balance.
        assert_eq!(order_.sell_amount, 5.into());
        assert_eq!(order_.buy_amount, 10.into());
        assert_eq!(order_.user_fee, 15.into());

        order.metadata.executed_sell_amount_before_fees = 0.into();
        let order_ = converter
            .normalize_limit_order(
                BalancedOrder {
                    order,
                    available_sell_token_balance: 20.into(),
                },
                true,
            )
            .unwrap();
        // Amounts are still quartered because of balance.
        assert_eq!(order_.sell_amount, 5.into());
        assert_eq!(order_.buy_amount, 10.into());
        assert_eq!(order_.user_fee, 15.into());
    }

    #[test]
    fn limit_orders_scaled_to_zero_amounts_rejected() {
        let converter = OrderConverter::test(Default::default());

        let sell = Order {
            data: OrderData {
                sell_amount: 100.into(),
                buy_amount: 10.into(),
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

        assert!(converter.normalize_limit_order(sell.clone(), true).is_ok());

        // Execute the order so that scaling the buy_amount would result in a
        // 0 amount.
        sell.order.metadata.executed_sell_amount = 99_u32.into();
        sell.order.metadata.executed_sell_amount_before_fees = 99_u32.into();
        sell.order.metadata.executed_buy_amount = 10_u32.into();

        assert!(converter.normalize_limit_order(sell, true).is_err());

        let buy = Order {
            data: OrderData {
                sell_amount: 10.into(),
                buy_amount: 100.into(),
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

        assert!(converter.normalize_limit_order(buy.clone(), true).is_ok());

        // Execute the order so that scaling the sell_amount would result in a
        // 0 amount.
        buy.order.metadata.executed_sell_amount = 10_u32.into();
        buy.order.metadata.executed_sell_amount_before_fees = 10_u32.into();
        buy.order.metadata.executed_buy_amount = 99_u32.into();

        assert!(converter.normalize_limit_order(buy, true).is_err());
    }
}
