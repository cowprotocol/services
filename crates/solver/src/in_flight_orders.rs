use crate::settlement::{Settlement, TradeExecution};
use itertools::Itertools;
use model::{
    auction::Auction,
    order::{Order, OrderUid},
};
use shared::conversions::u256_to_big_uint;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
struct PartiallyFilledOrder {
    order: Order,
    in_flight_trades: Vec<TradeExecution>,
}

impl PartiallyFilledOrder {
    pub fn order_with_remaining_amounts(&self) -> Order {
        let mut updated_order = self.order.clone();

        for trade in &self.in_flight_trades {
            updated_order.metadata.executed_buy_amount += u256_to_big_uint(&trade.buy_amount);
            updated_order.metadata.executed_sell_amount += u256_to_big_uint(&trade.sell_amount);
            updated_order.metadata.executed_fee_amount += trade.fee_amount;
        }

        updated_order
    }
}

/// After a settlement transaction we need to keep track of in flight orders until the api has
/// seen the tx. Otherwise we would attempt to solve already matched orders again leading to
/// failures.
#[derive(Default)]
pub struct InFlightOrders {
    /// Maps block to orders settled in that block.
    in_flight: BTreeMap<u64, Vec<OrderUid>>,
    /// Tracks in flight trades which use liquidity from partially fillable orders.
    in_flight_trades: HashMap<OrderUid, PartiallyFilledOrder>,
}

impl InFlightOrders {
    /// Takes note of the new set of solvable orders and returns the ones that aren't in flight and
    /// scales down partially fillable orders if there are currently orders in-flight tapping into
    /// their executable amounts.
    pub fn update_and_filter(&mut self, auction: &mut Auction) {
        // If api has seen block X then trades starting at X + 1 are still in flight.
        self.in_flight = self
            .in_flight
            .split_off(&(auction.latest_settlement_block + 1));

        let in_flight = self
            .in_flight
            .values()
            .flatten()
            .copied()
            .collect::<HashSet<_>>();

        self.in_flight_trades
            .retain(|uid, _| in_flight.contains(uid));

        auction.orders.iter_mut().for_each(|order| {
            let uid = &order.metadata.uid;

            if order.creation.partially_fillable {
                if let Some(trades) = self.in_flight_trades.get(uid) {
                    *order = trades.order_with_remaining_amounts();
                }
            } else if in_flight.contains(uid) {
                // fill-or-kill orders can only be used once and there is already a trade in flight
                // for this one => Modify it such that it gets filtered out in the next step.
                order.metadata.executed_buy_amount = u256_to_big_uint(&order.creation.buy_amount);
            }
        });
        auction.orders.retain(|order| {
            u256_to_big_uint(&order.creation.buy_amount) > order.metadata.executed_buy_amount
                && u256_to_big_uint(&order.creation.sell_amount)
                    > order.metadata.executed_sell_amount
        });
    }

    /// Tracks all in_flight orders and how much of the executable amount of partially fillable
    /// orders is currently used in in-flight trades.
    pub fn mark_settled_orders(&mut self, block: u64, settlement: &Settlement) {
        let mut uids = Vec::default();
        settlement
            .executed_trades()
            .inspect(|(trade, _)| {
                uids.push(trade.order.metadata.uid);
            })
            .filter(|(trade, _)| trade.order.creation.partially_fillable)
            .into_group_map_by(|(trade, _)| &trade.order)
            .into_iter()
            .for_each(|(order, trades)| {
                let uid = order.metadata.uid;
                let most_recent_data = PartiallyFilledOrder {
                    order: order.clone(),
                    in_flight_trades: trades.into_iter().map(|(_, trade)| trade).collect(),
                };
                // always overwrite existing data with the most recent data
                self.in_flight_trades.insert(uid, most_recent_data);
            });

        self.in_flight.entry(block).or_default().extend(uids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::{LiquidityOrderTrade, OrderTrade, SettlementEncoder, Trade};
    use maplit::hashmap;
    use model::order::{Order, OrderCreation, OrderKind, OrderMetadata};
    use primitive_types::H160;

    #[test]
    fn test() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);

        let fill_or_kill = Order {
            creation: OrderCreation {
                sell_token: token0,
                buy_token: token1,
                sell_amount: 100u8.into(),
                buy_amount: 100u8.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid: OrderUid::from_integer(1),
                ..Default::default()
            },
        };

        // partially fillable order 30% filled
        let mut partially_fillable_1 = fill_or_kill.clone();
        partially_fillable_1.creation.partially_fillable = true;
        partially_fillable_1.metadata.uid = OrderUid::from_integer(2);
        partially_fillable_1.metadata.executed_buy_amount = 30u8.into();
        partially_fillable_1.metadata.executed_sell_amount = 30u8.into();

        // a different partially fillable order 30% filled
        let mut partially_fillable_2 = partially_fillable_1.clone();
        partially_fillable_2.metadata.uid = OrderUid::from_integer(3);

        let user_trades = vec![OrderTrade {
            trade: Trade {
                order: fill_or_kill.clone(),
                ..Default::default()
            },
            ..Default::default()
        }];

        let liquidity_trades = vec![
            // This order uses some of the remaining executable amount of partially_fillable_1
            LiquidityOrderTrade {
                trade: Trade {
                    order: partially_fillable_2.clone(),
                    executed_amount: 20u8.into(),
                    ..Default::default()
                },
                buy_token_price: 1u8.into(),
                ..Default::default()
            },
            // Following orders use remaining executable amount of partially_fillable_2
            LiquidityOrderTrade {
                trade: Trade {
                    order: partially_fillable_1.clone(),
                    executed_amount: 50u8.into(),
                    ..Default::default()
                },
                buy_token_price: 1u8.into(),
                ..Default::default()
            },
            LiquidityOrderTrade {
                trade: Trade {
                    order: partially_fillable_1.clone(),
                    executed_amount: 20u8.into(),
                    ..Default::default()
                },
                buy_token_price: 1u8.into(),
                ..Default::default()
            },
        ];

        let prices = hashmap! {token0 => 1u8.into(), token1 => 1u8.into()};
        let settlement = Settlement {
            encoder: SettlementEncoder::with_trades(prices, user_trades, liquidity_trades),
        };

        let mut inflight = InFlightOrders::default();
        inflight.mark_settled_orders(1, &settlement);
        let mut order0 = fill_or_kill.clone();
        order0.metadata.uid = OrderUid::from_integer(0);
        let mut auction = Auction {
            block: 0,
            orders: vec![
                order0,
                fill_or_kill,
                partially_fillable_1,
                partially_fillable_2,
            ],
            ..Default::default()
        };

        let mut update_and_get_filtered_orders = |auction: &Auction| {
            let mut auction = auction.clone();
            inflight.update_and_filter(&mut auction);
            auction.orders
        };

        let filtered = update_and_get_filtered_orders(&auction);
        assert_eq!(filtered.len(), 2);
        // keep order 0 because there are no trades for it in flight
        assert_eq!(filtered[0].metadata.uid, OrderUid::from_integer(0));
        // drop order 1 because it's fill-or-kill and there is already one trade in flight
        // keep order 2 and reduce remaning executable amount by trade amounts currently in flight
        assert_eq!(filtered[1].metadata.uid, OrderUid::from_integer(3));
        assert_eq!(filtered[1].metadata.executed_buy_amount, 50u8.into());
        assert_eq!(filtered[1].metadata.executed_sell_amount, 50u8.into());
        // drop order 3 because in flight orders filled the remaining executable amount

        auction.block = 1;
        let filtered = update_and_get_filtered_orders(&auction);
        // same behaviour as above
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].metadata.uid, OrderUid::from_integer(0));
        assert_eq!(filtered[1].metadata.uid, OrderUid::from_integer(3));
        assert_eq!(filtered[1].metadata.executed_buy_amount, 50u8.into());
        assert_eq!(filtered[1].metadata.executed_sell_amount, 50u8.into());

        auction.latest_settlement_block = 1;
        let filtered = update_and_get_filtered_orders(&auction);
        // Because we drop all in-flight trades from blocks older than the settlement block there
        // is nothing left to filter solvable orders by => keep all orders unaltered
        assert_eq!(filtered.len(), 4);
    }
}
