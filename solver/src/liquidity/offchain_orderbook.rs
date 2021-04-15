use crate::orderbook::OrderBookApi;
use crate::settlement::{Interaction, Trade};
use anyhow::{Context, Result};
use contracts::WETH9;
use model::order::Order;
use primitive_types::U256;
use std::sync::Arc;

use super::{LimitOrder, LimitOrderSettlementHandling};

impl OrderBookApi {
    /// Returns a list of limit orders coming from the offchain orderbook API
    pub async fn get_liquidity(&self) -> Result<Vec<LimitOrder>> {
        Ok(self
            .get_orders()
            .await
            .context("failed to get orderbook")?
            .into_iter()
            .map(|order| normalize_limit_order(order, self.get_native_token()))
            .collect())
    }
}

struct OrderSettlementHandling {
    #[allow(dead_code)]
    native_token: WETH9,
    order: Order,
}

pub fn normalize_limit_order(order: Order, native_token: WETH9) -> LimitOrder {
    LimitOrder {
        id: order.order_meta_data.uid.to_string(),
        sell_token: order.order_creation.sell_token,
        // TODO handle ETH buy token address (0xe...e) by making the handler include an WETH.unwrap() interaction
        buy_token: order.order_creation.buy_token,
        // TODO discount previously executed sell amount
        sell_amount: order.order_creation.sell_amount,
        buy_amount: order.order_creation.buy_amount,
        kind: order.order_creation.kind,
        partially_fillable: order.order_creation.partially_fillable,
        settlement_handling: Arc::new(OrderSettlementHandling {
            order,
            native_token,
        }),
    }
}

impl LimitOrderSettlementHandling for OrderSettlementHandling {
    fn settle(&self, executed_amount: U256) -> (Option<Trade>, Vec<Box<dyn Interaction>>) {
        (
            Some(Trade::matched(self.order.clone(), executed_amount)),
            Vec::new(),
        )
    }
}
