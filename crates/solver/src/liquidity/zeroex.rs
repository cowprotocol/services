use super::SettlementHandling;
use crate::interactions::ZeroExInteraction;
use crate::liquidity::{Exchange, LimitOrder, Liquidity};
use crate::settlement::SettlementEncoder;
use anyhow::Result;
use contracts::{GPv2Settlement, IZeroEx};
use model::order::OrderKind;
use model::TokenPair;
use primitive_types::{H160, U256};
use shared::baseline_solver::BaseTokens;
use shared::zeroex_api::{Order, OrderRecord, OrdersQuery, ZeroExApi};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct ZeroExLiquidity {
    pub api: Arc<dyn ZeroExApi>,
    pub zeroex: IZeroEx,
    pub base_tokens: Arc<BaseTokens>,
    pub gpv2: GPv2Settlement,
}

type OrderBuckets = HashMap<(H160, H160), Vec<OrderRecord>>;

impl ZeroExLiquidity {
    pub async fn get_liquidity(&self, user_orders: &[LimitOrder]) -> Result<Vec<Liquidity>> {
        let queries = &[
            // orders fillable by anyone
            OrdersQuery::default(),
            // orders fillable only by our settlement contract
            OrdersQuery {
                sender: Some(self.gpv2.address()),
                ..Default::default()
            },
        ];

        let zeroex_orders_results =
            futures::future::join_all(queries.iter().map(|query| self.api.get_orders(query))).await;
        let zeroex_orders = zeroex_orders_results
            .into_iter()
            .flat_map(|result| match result {
                Ok(order_record_vec) => order_record_vec,
                Err(err) => {
                    tracing::warn!("ZeroExResponse error during liqudity fetching: {}", err);
                    vec![]
                }
            });

        let user_order_pairs = user_orders
            .iter()
            .filter_map(|order| TokenPair::new(order.buy_token, order.sell_token));
        let relevant_pairs = self.base_tokens.relevant_pairs(user_order_pairs);

        let order_buckets = self.generate_order_buckets(zeroex_orders, relevant_pairs);
        let filtered_zeroex_orders = self.get_useful_orders(order_buckets, 5);

        Ok(filtered_zeroex_orders)
    }

    fn generate_order_buckets(
        &self,
        zeroex_orders: impl Iterator<Item = OrderRecord>,
        relevant_pairs: HashSet<TokenPair>,
    ) -> OrderBuckets {
        // divide orders in buckets
        let mut buckets: OrderBuckets = Default::default();
        zeroex_orders
            .filter(|record| {
                match TokenPair::new(record.order.taker_token, record.order.maker_token) {
                    Some(pair) => relevant_pairs.contains(&pair),
                    None => false,
                }
            })
            .for_each(|order| {
                let bucket = buckets
                    .entry((order.order.maker_token, order.order.taker_token))
                    .or_insert(vec![]);
                bucket.push(order);
            });
        buckets
    }

    /// Get the `orders_per_type` best priced and biggest volume orders.
    fn get_useful_orders(
        &self,
        order_buckets: OrderBuckets,
        orders_per_type: usize,
    ) -> Vec<Liquidity> {
        let mut filtered_zeroex_orders = vec![];
        order_buckets.into_values().for_each(|mut orders| {
            if orders.len() <= 2 * orders_per_type {
                filtered_zeroex_orders.extend(orders);
                return;
            }
            // Sorting to have best priced orders at the end of the vector
            orders.sort_by(|order_1, order_2| {
                let price_1 = order_1.order.taker_amount as f64 / order_1.order.maker_amount as f64;
                let price_2 = order_2.order.taker_amount as f64 / order_2.order.maker_amount as f64;
                price_2.partial_cmp(&price_1).unwrap()
            });
            filtered_zeroex_orders.extend(orders.drain(orders.len() - orders_per_type..));

            orders.sort_by_key(|order| order.metadata.remaining_fillable_taker_amount);
            orders.reverse();
            filtered_zeroex_orders.extend(orders.into_iter().rev().take(orders_per_type));
        });
        let filtered_zeroex_orders: Vec<_> = filtered_zeroex_orders
            .into_iter()
            .flat_map(|order| self.record_into_liquidity(order))
            .collect();
        filtered_zeroex_orders
    }

    /// Turns 0x OrderRecord into liquidity which solvers can use.
    fn record_into_liquidity(&self, record: OrderRecord) -> Option<Liquidity> {
        let sell_amount: U256 = record.remaining_maker_amount().ok()?.into();
        if sell_amount.is_zero() || record.metadata.remaining_fillable_taker_amount == 0 {
            // filter out orders with 0 amounts to prevent errors in the solver
            return None;
        }

        let limit_order = LimitOrder {
            id: hex::encode(&record.metadata.order_hash),
            sell_token: record.order.maker_token,
            buy_token: record.order.taker_token,
            sell_amount,
            buy_amount: record.metadata.remaining_fillable_taker_amount.into(),
            kind: OrderKind::Buy,
            partially_fillable: true,
            unscaled_subsidized_fee: U256::zero(),
            scaled_unsubsidized_fee: U256::zero(),
            is_liquidity_order: true,
            settlement_handling: Arc::new(OrderSettlementHandler {
                order: record.order,
                zeroex: self.zeroex.clone(),
            }),
            exchange: Exchange::ZeroEx,
        };
        Some(Liquidity::LimitOrder(limit_order))
    }
}

struct OrderSettlementHandler {
    order: Order,
    zeroex: IZeroEx,
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn encode(&self, executed_amount: U256, encoder: &mut SettlementEncoder) -> Result<()> {
        if executed_amount > u128::MAX.into() {
            anyhow::bail!("0x only supports executed amounts of size u128");
        }
        encoder.append_to_execution_plan(ZeroExInteraction {
            taker_token_fill_amount: executed_amount.as_u128(),
            order: self.order.clone(),
            zeroex: self.zeroex.clone(),
        });
        Ok(())
    }
}
