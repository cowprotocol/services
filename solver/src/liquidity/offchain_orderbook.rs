use crate::orderbook::OrderBookApi;
use crate::settlement::{Interaction, Trade};
use anyhow::{Context, Result};
use model::order::{Order, OrderCreation};
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
            .map(|order| order.into())
            .collect())
    }
}

impl From<Order> for LimitOrder {
    fn from(order: Order) -> Self {
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
            settlement_handling: Arc::new(order.order_creation),
        }
    }
}

impl LimitOrderSettlementHandling for OrderCreation {
    fn settle(&self, executed_amount: U256) -> (Option<Trade>, Vec<Box<dyn Interaction>>) {
        (Some(Trade::matched(*self, executed_amount)), Vec::new())
    }
}
