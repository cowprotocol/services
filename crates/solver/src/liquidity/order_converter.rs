use {
    super::{Exchange, LimitOrder, LimitOrderId, LiquidityOrderId, SettlementHandling},
    crate::{interactions::UnwrapWethInteraction, settlement::SettlementEncoder},
    anyhow::{Context, Result},
    contracts::WETH9,
    ethcontract::U256,
    model::order::{LimitOrderClass, Order, OrderClass, BUY_ETH_ADDRESS},
    std::{sync::Arc, time::Duration},
};

pub struct OrderConverter {
    pub native_token: WETH9,
    pub min_order_age: Duration,
}

impl OrderConverter {
    /// Creates a order converter with the specified WETH9 address for unit
    /// testing purposes.
    #[cfg(test)]
    pub fn test(native_token: ethcontract::H160) -> Self {
        Self {
            native_token: shared::dummy_contract!(WETH9, native_token),
            min_order_age: Duration::from_secs(30),
        }
    }

    /// Converts a GPv2 order into a `LimitOrder` type liquidity for solvers.
    pub fn normalize_limit_order(&self, order: Order) -> Result<LimitOrder> {
        let native_token = self.native_token.clone();
        let buy_token = if order.data.buy_token == BUY_ETH_ADDRESS {
            native_token.address()
        } else {
            order.data.buy_token
        };

        let remaining = shared::remaining_amounts::Remaining::from_order(&order)?;

        // The reported fee amount that is used for objective computation is the
        // order's full full amount scaled by a constant factor.
        let solver_fee = remaining.remaining(order.metadata.solver_fee)?;
        let is_mature = order.metadata.creation_date
            + chrono::Duration::from_std(self.min_order_age).unwrap()
            <= chrono::offset::Utc::now();

        let sell_amount = match &order.metadata.class {
            OrderClass::Limit(limit) => {
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
        Ok(LimitOrder {
            id,
            sell_token: order.data.sell_token,
            buy_token,
            sell_amount: remaining.remaining(sell_amount)?,
            buy_amount: remaining.remaining(order.data.buy_amount)?,
            kind: order.data.kind,
            partially_fillable: order.data.partially_fillable,
            solver_fee,
            settlement_handling: Arc::new(OrderSettlementHandler {
                order,
                native_token,
                solver_fee,
            }),
            exchange: Exchange::GnosisProtocol,
            // TODO: It would be nicer to set this here too but we need #529 first.
            reward: 0.,
            is_mature,
        })
    }
}

struct OrderSettlementHandler {
    order: Order,
    native_token: WETH9,
    solver_fee: U256,
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

    fn encode(&self, executed_amount: U256, encoder: &mut SettlementEncoder) -> Result<()> {
        let is_native_token_buy_order = self.order.data.buy_token == BUY_ETH_ADDRESS;
        if is_native_token_buy_order {
            encoder.add_token_equivalency(self.native_token.address(), BUY_ETH_ADDRESS)?;
        }

        let trade = encoder.add_trade(self.order.clone(), executed_amount, self.solver_fee)?;

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
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            converter.normalize_limit_order(order).unwrap().buy_token,
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
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            converter.normalize_limit_order(order).unwrap().buy_token,
            buy_token
        );
    }

    #[test]
    fn adds_unwrap_interaction_for_sell_order_with_eth_flag() {
        let native_token_address = H160([0x42; 20]);
        let sell_token = H160([0x21; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);

        let executed_amount = U256::from(1337);
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
            solver_fee,
        };

        assert_settlement_encoded_with(
            prices,
            order_settlement_handler,
            executed_amount,
            |encoder| {
                encoder
                    .add_token_equivalency(native_token.address(), BUY_ETH_ADDRESS)
                    .unwrap();
                encoder.add_unwrap(UnwrapWethInteraction {
                    weth: native_token,
                    amount: executed_buy_amount,
                });
                assert!(encoder
                    .add_trade(order, executed_amount, solver_fee)
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
            let executed_amount = U256::from(1337);
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
                solver_fee: 0.into(),
            };

            assert_settlement_encoded_with(
                prices,
                order_settlement_handler,
                executed_amount,
                |encoder| {
                    encoder
                        .add_token_equivalency(native_token.address(), BUY_ETH_ADDRESS)
                        .unwrap();
                    assert!(encoder.add_trade(order, executed_amount, 0.into()).is_ok());
                    encoder.add_unwrap(UnwrapWethInteraction {
                        weth: native_token,
                        amount: executed_amount,
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

        let executed_amount = U256::from(1337);
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
            solver_fee: 0.into(),
        };

        assert_settlement_encoded_with(
            prices,
            order_settlement_handler,
            executed_amount,
            |encoder| {
                assert!(encoder.add_trade(order, executed_amount, 0.into()).is_ok());
            },
        );
    }

    #[test]
    fn scales_limit_order_amounts_for_partially_filled_orders() {
        let converter = OrderConverter::test(H160::default());
        let order = converter
            .normalize_limit_order(Order {
                data: OrderData {
                    sell_amount: 10.into(),
                    buy_amount: 20.into(),
                    fee_amount: 30.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: true,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    executed_sell_amount_before_fees: 5.into(),
                    solver_fee: 200.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();

        assert_eq!(order.sell_amount, 5.into());
        assert_eq!(order.buy_amount, 10.into());
        assert_eq!(order.solver_fee, 100.into());
    }

    #[test]
    fn limit_orders_get_adjusted_for_surplus_fee() {
        let converter = OrderConverter::test(Default::default());
        let order = OrderBuilder::default()
            .with_class(OrderClass::Limit(Default::default()))
            .with_sell_amount(1_000.into())
            .with_fee_amount(200.into())
            .with_surplus_fee(100.into())
            .with_solver_fee(200.into())
            .build();
        let solver_order = converter.normalize_limit_order(order).unwrap();

        // sell_amount + fee_amount - surplus_fee = 1_000 + 200 - 100
        assert_eq!(solver_order.sell_amount, 1_100.into());
        // it's the `autopilot`'s responsibility to prepare this value for us so we
        // don't touch it
        assert_eq!(solver_order.solver_fee, 200.into());
    }
}
