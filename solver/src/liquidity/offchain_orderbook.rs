use crate::orderbook::OrderBookApi;
use crate::settlement::SettlementEncoder;
use anyhow::{Context, Result};
use contracts::WETH9;
use ethcontract::H160;
use model::order::Order;
use primitive_types::U256;
use std::sync::Arc;

use super::{LimitOrder, SettlementHandling};

pub const BUY_ETH_ADDRESS: H160 = H160([0xee; 20]);

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

struct OrderSettlementHandler {
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
        settlement_handling: Arc::new(OrderSettlementHandler {
            order,
            native_token,
        }),
    }
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn encode(&self, executed_amount: U256, encoder: &mut SettlementEncoder) -> Result<()> {
        encoder.add_trade(self.order.clone(), executed_amount)
    }
}
