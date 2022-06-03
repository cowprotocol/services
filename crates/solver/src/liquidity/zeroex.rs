use super::SettlementHandling;
use crate::interactions::ZeroExInteraction;
use crate::liquidity::{Exchange, LimitOrder, Liquidity};
use crate::settlement::SettlementEncoder;
use anyhow::Result;
use contracts::{GPv2Settlement, IZeroEx};
use model::order::OrderKind;
use model::TokenPair;
// use hex_literal::hex;
use primitive_types::{H160, U256};
use shared::baseline_solver::BaseTokens;
use shared::zeroex_api::{Order, OrderRecord, OrdersQuery, ZeroExApi};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ZeroExLiquidity {
    pub api: Arc<dyn ZeroExApi>,
    pub zeroex: IZeroEx,
    pub base_tokens: Arc<BaseTokens>,
    pub gpv2: GPv2Settlement,
}

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
        // let fake_order = [TokenPair::new(
        //     H160(hex!("2b591e99afe9f32eaa6214f7b7629768c40eeb39")),
        //     H160::default(),
        // )
        // .unwrap()]
        // .into_iter();

        let user_order_pairs = user_orders
            .iter()
            .filter_map(|order| TokenPair::new(order.buy_token, order.sell_token));
        let relevant_pairs = self.base_tokens.relevant_pairs(user_order_pairs);

        // divide orders in buckets and choose best orders in those buckets
        let mut order_buckets: HashMap<(H160, H160), Vec<OrderRecord>> = HashMap::default();

        zeroex_orders
            .filter(|record| {
                match TokenPair::new(record.order.taker_token, record.order.maker_token) {
                    Some(pair) => relevant_pairs.contains(&pair),
                    None => false,
                }
            })
            .for_each(|order| {
                let orders_list = order_buckets
                    .entry((order.order.maker_token, order.order.taker_token))
                    .or_insert(vec![]);
                orders_list.push(order);
            });
        let mut limit_orders_n = vec![];
        order_buckets.into_values().for_each(|mut order_list| {
            order_list.sort_by(|order_1, order_2| {
                let ratio_1 = order_1.order.taker_amount / order_1.order.maker_amount;
                let ratio_2 = order_2.order.taker_amount / order_2.order.maker_amount;
                ratio_1.cmp(&ratio_2)
            });
            let mut copy_of_list = order_list.clone();
            limit_orders_n.extend(order_list.into_iter().take(5));

            if copy_of_list.len() > 5 {
                copy_of_list.drain(0..5);
            }
            copy_of_list.sort_by_key(|order| order.metadata.remaining_fillable_taker_amount);
            copy_of_list.reverse();
            limit_orders_n.extend(copy_of_list.iter().take(5).cloned());
        });
        let mut limit_orders_list: Vec<LimitOrder> = limit_orders_n
            .into_iter()
            .flat_map(|order| self.convert_to_limit_order(order))
            .collect();
        limit_orders_list.sort_by_key(|order| order.id.clone());
        limit_orders_list.dedup();
        let filtered_zeroex_orders: Vec<Liquidity> = limit_orders_list
            .iter()
            .filter_map(|record| self.record_into_liquidity(record.clone()))
            .collect();
        println!("Number of zeroex orders {:?}", filtered_zeroex_orders.len());

        Ok(filtered_zeroex_orders)
    }

    // Turns 0x OrderRecord into LimitOrder for sorting and deduplication
    fn convert_to_limit_order(&self, record: OrderRecord) -> Option<LimitOrder> {
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
        Some(limit_order)
    }

    // Turns 0x LimitOrder into liquidity which solvers can use.
    fn record_into_liquidity(&self, record: LimitOrder) -> Option<Liquidity> {
        Some(Liquidity::LimitOrder(record))
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
