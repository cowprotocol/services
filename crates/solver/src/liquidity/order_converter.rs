use super::{LimitOrder, SettlementHandling};
use crate::{interactions::UnwrapWethInteraction, settlement::SettlementEncoder};
use anyhow::Result;
use contracts::WETH9;
use ethcontract::{H160, U256};
use model::order::{Order, BUY_ETH_ADDRESS};
use std::{collections::HashSet, sync::Arc};

pub struct OrderConverter {
    pub native_token: WETH9,
    pub liquidity_order_owners: HashSet<H160>,
    pub fee_objective_scaling_factor: f64,
}

impl OrderConverter {
    /// Creates a order converter with the specified WETH9 address for unit
    /// testing purposes.
    #[cfg(test)]
    pub fn test(native_token: H160) -> Self {
        Self {
            native_token: shared::dummy_contract!(WETH9, native_token),
            liquidity_order_owners: HashSet::new(),
            fee_objective_scaling_factor: 1.,
        }
    }

    /// Converts a GPv2 order into a `LimitOrder` type liquidity for solvers.
    pub fn normalize_limit_order(&self, order: Order) -> LimitOrder {
        let native_token = self.native_token.clone();
        let buy_token = if order.order_creation.buy_token == BUY_ETH_ADDRESS {
            native_token.address()
        } else {
            order.order_creation.buy_token
        };

        // The reported fee amount that is used for objective computation is the
        // order's full full amount scaled by a constant factor.
        let scaled_fee_amount = U256::from_f64_lossy(
            order.order_meta_data.full_fee_amount.to_f64_lossy()
                * self.fee_objective_scaling_factor,
        );
        let is_liquidity_order = self
            .liquidity_order_owners
            .contains(&order.order_meta_data.owner);
        LimitOrder {
            id: order.order_meta_data.uid.to_string(),
            sell_token: order.order_creation.sell_token,
            buy_token,
            // TODO discount previously executed sell amount
            // https://github.com/gnosis/gp-v2-services/issues/673
            sell_amount: order.order_creation.sell_amount,
            buy_amount: order.order_creation.buy_amount,
            kind: order.order_creation.kind,
            partially_fillable: order.order_creation.partially_fillable,
            unscaled_subsidized_fee: order.order_creation.fee_amount,
            scaled_unsubsidized_fee: scaled_fee_amount,
            is_liquidity_order,
            settlement_handling: Arc::new(OrderSettlementHandler {
                order,
                native_token,
                scaled_unsubsidized_fee_amount: scaled_fee_amount,
                is_liquidity_order,
            }),
        }
    }
}

struct OrderSettlementHandler {
    order: Order,
    native_token: WETH9,
    scaled_unsubsidized_fee_amount: U256,
    is_liquidity_order: bool,
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn encode(&self, executed_amount: U256, encoder: &mut SettlementEncoder) -> Result<()> {
        let is_native_token_buy_order = self.order.order_creation.buy_token == BUY_ETH_ADDRESS;

        if is_native_token_buy_order {
            encoder.add_token_equivalency(self.native_token.address(), BUY_ETH_ADDRESS)?;
        }

        let trade = encoder.add_trade(
            self.order.clone(),
            executed_amount,
            self.order.order_creation.fee_amount,
            self.scaled_unsubsidized_fee_amount,
            self.is_liquidity_order,
        )?;

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
    use super::*;
    use crate::settlement::tests::assert_settlement_encoded_with;
    use ethcontract::H160;
    use maplit::hashmap;
    use model::order::{OrderCreation, OrderKind, OrderMetaData};
    use shared::dummy_contract;

    #[test]
    fn eth_buy_liquidity_is_assigned_to_weth() {
        let native_token = H160([0x42; 20]);
        let converter = OrderConverter::test(native_token);
        let order = Order {
            order_creation: OrderCreation {
                buy_token: BUY_ETH_ADDRESS,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            converter.normalize_limit_order(order).buy_token,
            native_token,
        );
    }

    #[test]
    fn non_eth_buy_liquidity_stays_put() {
        let buy_token = H160([0x21; 20]);
        let converter = OrderConverter::test(H160([0x42; 20]));
        let order = Order {
            order_creation: OrderCreation {
                buy_token,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(converter.normalize_limit_order(order).buy_token, buy_token);
    }

    #[test]
    fn applies_objective_scaling_factor() {
        let converter = OrderConverter {
            fee_objective_scaling_factor: 1.5,
            ..OrderConverter::test(H160::default())
        };

        assert_eq!(
            converter
                .normalize_limit_order(Order {
                    order_creation: OrderCreation {
                        fee_amount: 10.into(),
                        ..Default::default()
                    },
                    order_meta_data: OrderMetaData {
                        full_fee_amount: 20.into(),
                        ..Default::default()
                    }
                })
                .scaled_unsubsidized_fee,
            30.into(),
        );

        assert_eq!(
            converter
                .normalize_limit_order(Order {
                    order_creation: OrderCreation {
                        fee_amount: 10.into(),
                        ..Default::default()
                    },
                    order_meta_data: OrderMetaData {
                        full_fee_amount: 50.into(),
                        ..Default::default()
                    },
                },)
                .scaled_unsubsidized_fee,
            75.into(),
        );
    }

    #[test]
    fn adds_unwrap_interaction_for_sell_order_with_eth_flag() {
        let native_token_address = H160([0x42; 20]);
        let sell_token = H160([0x21; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);

        let executed_amount = U256::from(1337);
        let executed_buy_amount = U256::from(2 * 1337);
        let scaled_fee_amount = U256::from(1234);

        let prices = hashmap! {
            native_token.address() => U256::from(100),
            sell_token => U256::from(200),
        };
        let order = Order {
            order_creation: OrderCreation {
                buy_token: BUY_ETH_ADDRESS,
                sell_token,
                sell_amount: 1337.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };
        println!("{}", order.order_creation.buy_token);

        let order_settlement_handler = OrderSettlementHandler {
            order: order.clone(),
            native_token: native_token.clone(),
            scaled_unsubsidized_fee_amount: scaled_fee_amount,
            is_liquidity_order: false,
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
                    .add_trade(order, executed_amount, scaled_fee_amount, 0.into(), false)
                    .is_ok());
            },
        );
    }

    #[test]
    fn adds_unwrap_interaction_for_buy_order_with_eth_flag() {
        let native_token_address = H160([0x42; 20]);
        let sell_token = H160([0x21; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);
        let executed_amount = U256::from(1337);
        let prices = hashmap! {
            native_token.address() => U256::from(1),
            sell_token => U256::from(2),
        };
        let order = Order {
            order_creation: OrderCreation {
                buy_token: BUY_ETH_ADDRESS,
                buy_amount: 1337.into(),
                sell_token,
                kind: OrderKind::Buy,
                ..Default::default()
            },
            ..Default::default()
        };
        println!("{}", order.order_creation.buy_token);

        let order_settlement_handler = OrderSettlementHandler {
            order: order.clone(),
            native_token: native_token.clone(),
            scaled_unsubsidized_fee_amount: 0.into(),
            is_liquidity_order: false,
        };

        assert_settlement_encoded_with(
            prices,
            order_settlement_handler,
            executed_amount,
            |encoder| {
                encoder
                    .add_token_equivalency(native_token.address(), BUY_ETH_ADDRESS)
                    .unwrap();
                assert!(encoder
                    .add_trade(order, executed_amount, 0.into(), 0.into(), false)
                    .is_ok());
                encoder.add_unwrap(UnwrapWethInteraction {
                    weth: native_token,
                    amount: executed_amount,
                });
            },
        );
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
            order_creation: OrderCreation {
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
            scaled_unsubsidized_fee_amount: 0.into(),
            is_liquidity_order: false,
        };

        assert_settlement_encoded_with(
            prices,
            order_settlement_handler,
            executed_amount,
            |encoder| {
                assert!(encoder
                    .add_trade(order, executed_amount, 0.into(), 0.into(), false)
                    .is_ok());
            },
        );
    }
}
