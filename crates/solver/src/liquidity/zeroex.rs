use super::SettlementHandling;
use crate::interactions::ZeroExInteraction;
use crate::liquidity::{Exchange, LimitOrder, Liquidity};
use crate::settlement::SettlementEncoder;
use anyhow::Result;
use contracts::{GPv2Settlement, IZeroEx};
use model::order::OrderKind;
use model::TokenPair;
use primitive_types::U256;
use shared::baseline_solver::BaseTokens;
use shared::zeroex_api::{Order, OrderRecord, OrdersQuery, ZeroExApi};
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

        let user_order_pairs = user_orders
            .iter()
            .filter_map(|order| TokenPair::new(order.buy_token, order.sell_token));
        let relevant_pairs = self.base_tokens.relevant_pairs(user_order_pairs);

        let filtered_zeroex_orders = zeroex_orders
            .filter(|record| {
                match TokenPair::new(record.order.taker_token, record.order.maker_token) {
                    Some(pair) => relevant_pairs.contains(&pair),
                    None => false,
                }
            })
            .filter_map(|record| self.record_into_liquidity(record))
            .collect();

        Ok(filtered_zeroex_orders)
    }

    /// Turns 0x OrderRecord into liquidity which solvers can use.
    fn record_into_liquidity(&self, record: OrderRecord) -> Option<Liquidity> {
        let sell_amount: U256 = record.remaining_maker_amount().ok()?.into();
        if sell_amount.is_zero() || record.metadata.remaining_fillable_taker_amount == 0 {
            // filter out orders with 0 amounts to prevent errors in the solver
            return None;
        }

        let limit_order = LimitOrder {
            id: hex::encode(&record.metadata.order_hash.0),
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
