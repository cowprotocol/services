use crate::interactions::UnwrapWethInteraction;
use crate::orderbook::OrderBookApi;
use crate::settlement::SettlementEncoder;
use anyhow::{anyhow, Context, Result};
use contracts::WETH9;
use ethcontract::H160;
use model::order::{Order, OrderKind, OrderUid, BUY_ETH_ADDRESS};
use primitive_types::U256;
use std::{collections::HashMap, sync::Arc};

use super::{LimitOrder, SettlementHandling};
use std::collections::HashSet;

impl OrderBookApi {
    /// Returns a list of limit orders coming from the offchain orderbook API
    pub async fn get_liquidity(
        &self,
        inflight_trades: &HashSet<OrderUid>,
    ) -> Result<Vec<LimitOrder>> {
        Ok(self
            .get_orders()
            .await
            .context("failed to get orderbook")?
            .into_iter()
            .filter_map(|order| inflight_order_filter(order, inflight_trades))
            .map(|altered_order| normalize_limit_order(altered_order, self.get_native_token()))
            .collect())
    }
}

struct OrderSettlementHandler {
    native_token: WETH9,
    order: Order,
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

pub fn normalize_limit_order(order: Order, native_token: WETH9) -> LimitOrder {
    let buy_token = if order.order_creation.buy_token == BUY_ETH_ADDRESS {
        native_token.address()
    } else {
        order.order_creation.buy_token
    };
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
        fee_amount: order.order_creation.fee_amount,
        settlement_handling: Arc::new(OrderSettlementHandler {
            native_token,
            order,
        }),
    }
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn encode(&self, executed_amount: U256, encoder: &mut SettlementEncoder) -> Result<()> {
        if self.order.order_creation.buy_token == BUY_ETH_ADDRESS {
            let interaction = compute_unwrap_interaction(
                encoder.clearing_prices(),
                &self.order,
                executed_amount,
                self.native_token.clone(),
            )?;

            encoder.add_token_equivalency(self.native_token.address(), BUY_ETH_ADDRESS)?;
            encoder.add_unwrap(interaction);
        }
        encoder.add_trade(self.order.clone(), executed_amount)
    }
}

fn compute_unwrap_interaction(
    clearing_prices: &HashMap<H160, U256>,
    order: &Order,
    executed_amount: U256,
    weth: WETH9,
) -> Result<UnwrapWethInteraction> {
    let sell_price = *clearing_prices
        .get(&order.order_creation.sell_token)
        .ok_or_else(|| anyhow!("sell price not available"))?;
    let buy_price = *clearing_prices
        .get(&weth.address())
        .ok_or_else(|| anyhow!("buy price not available"))?;
    let amount = executed_buy_amount(
        order,
        executed_amount,
        Price {
            sell_price,
            buy_price,
        },
    )
    .ok_or_else(|| anyhow!("cannot compute executed buy amount"))?;
    Ok(UnwrapWethInteraction { weth, amount })
}

struct Price {
    sell_price: U256,
    buy_price: U256,
}

fn executed_buy_amount(order: &Order, executed_amount: U256, price: Price) -> Option<U256> {
    Some(match order.order_creation.kind {
        OrderKind::Sell => executed_amount
            .checked_mul(price.sell_price)?
            .checked_div(price.buy_price)?,
        OrderKind::Buy => executed_amount,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::settlement::tests::assert_settlement_encoded_with;
    use maplit::{hashmap, hashset};
    use model::order::OrderCreation;
    use model::DomainSeparator;
    use shared::dummy_contract;

    #[test]
    fn eth_buy_liquidity_is_assigned_to_weth() {
        let native_token_address = H160([0x42; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);
        let order = Order {
            order_creation: OrderCreation {
                buy_token: BUY_ETH_ADDRESS,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            normalize_limit_order(order, native_token).buy_token,
            native_token_address
        );
    }
    #[test]
    fn non_eth_buy_liquidity_stays_put() {
        let buy_token = H160([0x21; 20]);
        let native_token_address = H160([0x42; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);
        let order = Order {
            order_creation: OrderCreation {
                buy_token,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            normalize_limit_order(order, native_token).buy_token,
            buy_token
        );
    }

    #[test]
    fn executed_buy_amount_returns_err_on_overflows() {
        let order = Order {
            order_creation: OrderCreation {
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };

        // mul
        let executed_amount = U256::MAX;
        let sell_price = U256::from(2);
        let buy_price = U256::one();
        assert!(executed_buy_amount(
            &order,
            executed_amount,
            Price {
                sell_price,
                buy_price
            }
        )
        .is_none());

        // div
        let executed_amount = U256::one();
        let sell_price = U256::one();
        let buy_price = U256::zero();
        assert!(executed_buy_amount(
            &order,
            executed_amount,
            Price {
                sell_price,
                buy_price
            }
        )
        .is_none());
    }

    #[test]
    fn adds_unwrap_interaction_for_sell_order_with_eth_flag() {
        let native_token_address = H160([0x42; 20]);
        let sell_token = H160([0x21; 20]);
        let native_token = dummy_contract!(WETH9, native_token_address);

        let executed_amount = U256::from(1337);
        let prices = hashmap! {
            native_token.address() => U256::from(100),
            sell_token => U256::from(200),
        };
        let executed_buy_amount = U256::from(2 * 1337);
        let order = Order {
            order_creation: OrderCreation {
                buy_token: BUY_ETH_ADDRESS,
                sell_token,
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };
        println!("{}", order.order_creation.buy_token);

        let order_settlement_handler = OrderSettlementHandler {
            order: order.clone(),
            native_token: native_token.clone(),
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
                assert!(encoder.add_trade(order, executed_amount).is_ok());
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
        };

        assert_settlement_encoded_with(
            prices,
            order_settlement_handler,
            executed_amount,
            |encoder| {
                encoder
                    .add_token_equivalency(native_token.address(), BUY_ETH_ADDRESS)
                    .unwrap();
                assert!(encoder.add_trade(order, executed_amount).is_ok());
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
                sell_token,
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
            executed_amount,
            |encoder| {
                assert!(encoder.add_trade(order, executed_amount).is_ok());
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
