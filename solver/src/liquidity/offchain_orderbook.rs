use super::{LimitOrder, SettlementHandling};
use crate::{
    interactions::UnwrapWethInteraction, orderbook::OrderBookApi, settlement::SettlementEncoder,
};
use anyhow::{Context as _, Result};
use contracts::WETH9;
use ethcontract::{H160, U256};
use model::order::{Order, OrderUid, BUY_ETH_ADDRESS};
use reqwest::{Client, Url};
use std::{collections::HashSet, sync::Arc};

pub struct OrderbookLiquidity {
    api: OrderBookApi,
    converter: OrderConverter,
}

impl OrderbookLiquidity {
    /// Creates a new liquidity fetcher for GPv2 off-chain orderbook.
    pub fn new(
        base: Url,
        client: Client,
        native_token: WETH9,
        liquidity_order_owners: HashSet<H160>,
        fee_factor: f64,
        fee_objective_scaling_factor: f64,
    ) -> Self {
        Self {
            api: OrderBookApi::new(base, client),
            converter: OrderConverter {
                native_token,
                liquidity_order_owners,
                fee_factor,
                fee_objective_scaling_factor,
            },
        }
    }

    /// Returns a list of limit orders coming from the offchain orderbook API
    pub async fn get_liquidity(
        &self,
        inflight_trades: &HashSet<OrderUid>,
    ) -> Result<Vec<LimitOrder>> {
        Ok(self
            .api
            .get_orders()
            .await
            .context("failed to get orderbook")?
            .into_iter()
            .filter_map(|order| inflight_order_filter(order, inflight_trades))
            .map(|altered_order| self.converter.normalize_limit_order(altered_order))
            .collect())
    }
}

fn inflight_order_filter(order: Order, inflight_trades: &HashSet<OrderUid>) -> Option<Order> {
    // TODO - could model inflight_trades as HashMap<OrderUid, Vec<Trade>>
    // https://github.com/gnosis/gp-v2-services/issues/673
    if inflight_trades.contains(&order.order_meta_data.uid) {
        return if order.order_creation.partially_fillable {
            // TODO - driver logic for Partially Fillable Orders
            // https://github.com/gnosis/gp-v2-services/issues/673
            // Note that this will result in simulation error "GPv2: order filled" if the
            // next solver run loop tries to match the order again beyond its remaining amount.
            Some(order)
        } else {
            // Fully filled, inflight orders are excluded from consideration
            None
        };
    }
    Some(order)
}

pub struct OrderConverter {
    native_token: WETH9,
    liquidity_order_owners: HashSet<H160>,
    fee_factor: f64,
    fee_objective_scaling_factor: f64,
}

impl OrderConverter {
    /// Creates a order converter with the specified WETH9 address for unit
    /// testing purposes.
    #[cfg(test)]
    pub fn test(native_token: H160) -> Self {
        Self {
            native_token: shared::dummy_contract!(WETH9, native_token),
            liquidity_order_owners: HashSet::new(),
            fee_factor: 1.,
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

        // In order to maintain backwards compatibility with orders created
        // before the full fee amount was recorded, guess the full amount based
        // on the actual fee amount and the global fee scaling factor. This can
        // be removed once there are no more active orders without a full fee
        // amount.
        // <https://github.com/gnosis/gp-v2-services/issues/1219>
        let full_fee_amount = if order.order_meta_data.full_fee_amount.is_zero() {
            U256::from_f64_lossy(order.order_creation.fee_amount.to_f64_lossy() / self.fee_factor)
        } else {
            order.order_meta_data.full_fee_amount
        };

        // The reported fee amount that is used for objective computation is the
        // order's full full amount scaled by a constant factor.
        let scaled_fee_amount = U256::from_f64_lossy(
            full_fee_amount.to_f64_lossy() * self.fee_objective_scaling_factor,
        );

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
            scaled_fee_amount,
            is_liquidity_order: self
                .liquidity_order_owners
                .contains(&order.order_meta_data.owner),
            settlement_handling: Arc::new(OrderSettlementHandler {
                order,
                native_token,
                scaled_fee_amount,
            }),
        }
    }
}

struct OrderSettlementHandler {
    order: Order,
    native_token: WETH9,
    scaled_fee_amount: U256,
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn encode(&self, executed_amount: U256, encoder: &mut SettlementEncoder) -> Result<()> {
        let is_native_token_buy_order = self.order.order_creation.buy_token == BUY_ETH_ADDRESS;

        if is_native_token_buy_order {
            encoder.add_token_equivalency(self.native_token.address(), BUY_ETH_ADDRESS)?;
        }

        let trade =
            encoder.add_trade(self.order.clone(), executed_amount, self.scaled_fee_amount)?;

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
    use maplit::{hashmap, hashset};
    use model::{
        order::{OrderCreation, OrderKind, OrderMetaData},
        DomainSeparator,
    };
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
    fn computes_full_fee_amount_if_missing() {
        let converter = OrderConverter {
            fee_factor: 0.5,
            ..OrderConverter::test(H160::default())
        };

        assert_eq!(
            converter
                .normalize_limit_order(Order {
                    order_creation: OrderCreation {
                        fee_amount: 10.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .scaled_fee_amount,
            20.into(),
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
                })
                .scaled_fee_amount,
            50.into(),
        );
    }

    #[test]
    fn applies_objective_scaling_factor() {
        let converter = OrderConverter {
            fee_factor: 0.5,
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
                    ..Default::default()
                })
                .scaled_fee_amount,
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
                .scaled_fee_amount,
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
            scaled_fee_amount,
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
                    .add_trade(order, executed_amount, scaled_fee_amount)
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
            scaled_fee_amount: 0.into(),
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
            scaled_fee_amount: 0.into(),
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
    fn inflight_order_filter_() {
        let fully_fillable_order = Order::from_order_creation(
            OrderCreation {
                partially_fillable: false,
                ..Default::default()
            },
            &DomainSeparator::default(),
            H160::default(),
            U256::default(),
        )
        .unwrap();
        assert!(inflight_order_filter(
            fully_fillable_order.clone(),
            &hashset!(fully_fillable_order.order_meta_data.uid)
        )
        .is_none());
        let order = inflight_order_filter(fully_fillable_order.clone(), &hashset!());
        assert!(order.is_some());
        assert_eq!(order.unwrap(), fully_fillable_order);

        let partially_fillable_order = Order::from_order_creation(
            OrderCreation {
                partially_fillable: true,
                ..Default::default()
            },
            &DomainSeparator::default(),
            H160::default(),
            U256::default(),
        )
        .unwrap();
        let adjusted_order = inflight_order_filter(
            partially_fillable_order.clone(),
            &hashset!(partially_fillable_order.order_meta_data.uid),
        );
        assert!(adjusted_order.is_some());

        // TODO - The following assertion will fail and need to be adapted in
        // https://github.com/gnosis/gp-v2-services/issues/673
        assert_eq!(adjusted_order.unwrap(), partially_fillable_order);
    }
}
