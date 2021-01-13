use crate::encoding;
use anyhow::Result;
use model::order::OrderCreation;
use primitive_types::{H160, U256};
use std::{
    collections::HashMap,
    io::{Cursor, Write},
};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub order: OrderCreation,
    pub executed_amount: U256,
    pub fee_discount: u16,
}

impl Trade {
    pub fn fully_matched(order: OrderCreation) -> Self {
        let executed_amount = match order.kind {
            model::order::OrderKind::Buy => order.buy_amount,
            model::order::OrderKind::Sell => order.sell_amount,
        };
        Self {
            order,
            executed_amount,
            fee_discount: 0,
        }
    }
}

pub trait Interaction: std::fmt::Debug {
    // TODO: not sure if this should return a result.
    // Write::write returns a result but we know we write to a vector in memory so we know it will
    // never fail. Then the question becomes whether interactions should be allowed to fail encoding
    // for other reasons.
    fn encode(&self, writer: &mut dyn Write) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct Settlement {
    pub clearing_prices: HashMap<H160, U256>,
    pub fee_factor: U256,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Box<dyn Interaction>>,
    pub order_refunds: Vec<()>,
}

impl Settlement {
    pub fn tokens(&self) -> Vec<H160> {
        self.clearing_prices.keys().copied().collect()
    }

    pub fn clearing_prices(&self) -> Vec<U256> {
        self.clearing_prices.values().copied().collect()
    }

    // Returns None if a trade uses a token for which there is no price.
    pub fn encode_trades(&self) -> Option<Vec<u8>> {
        let mut token_index = HashMap::new();
        for (i, token) in self.clearing_prices.keys().enumerate() {
            token_index.insert(token, i as u8);
        }
        let mut bytes = Vec::with_capacity(encoding::TRADE_STRIDE * self.trades.len());
        for trade in &self.trades {
            let order = &trade.order;
            let encoded = encoding::encode_trade(
                &order,
                *token_index.get(&order.sell_token)?,
                *token_index.get(&order.buy_token)?,
                &trade.executed_amount,
                trade.fee_discount,
            );
            bytes.extend_from_slice(&encoded);
        }
        Some(bytes)
    }

    pub fn encode_interactions(&self) -> Result<Vec<u8>> {
        let mut cursor = Cursor::new(Vec::new());
        for interaction in &self.interactions {
            interaction.encode(&mut cursor)?;
        }
        Ok(cursor.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn encode_trades_finds_token_index() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let order0 = OrderCreation {
            sell_token: token0,
            buy_token: token1,
            ..Default::default()
        };
        let order1 = OrderCreation {
            sell_token: token1,
            buy_token: token0,
            ..Default::default()
        };
        let trade0 = Trade {
            order: order0,
            ..Default::default()
        };
        let trade1 = Trade {
            order: order1,
            ..Default::default()
        };
        let settlement = Settlement {
            clearing_prices: maplit::hashmap! {token0 => 0.into(), token1 => 0.into()},
            trades: vec![trade0, trade1],
            ..Default::default()
        };
        assert!(settlement.encode_trades().is_some());
    }
}
