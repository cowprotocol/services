use crate::orderbook::OrderBookApi;
use crate::settlement::{Interaction, Trade};
use anyhow::{Context, Result};
use model::order::OrderCreation;
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
            .map(|order| order.order_creation.into())
            .collect())
    }
}

impl From<OrderCreation> for LimitOrder {
    fn from(order: OrderCreation) -> Self {
        LimitOrder {
            sell_token: order.sell_token,
            // TODO handle ETH buy token address (0xe...e) by making the handler include an WETH.unwrap() interaction
            buy_token: order.buy_token,
            // TODO discount previously executed sell amount
            sell_amount: order.sell_amount,
            buy_amount: order.buy_amount,
            kind: order.kind,
            partially_fillable: order.partially_fillable,
            settlement_handling: Arc::new(order),
        }
    }
}

impl LimitOrderSettlementHandling for OrderCreation {
    fn settle(&self, executed_amount: U256) -> (Option<Trade>, Vec<Box<dyn Interaction>>) {
        (Some(Trade::matched(*self, executed_amount)), Vec::new())
    }
}
