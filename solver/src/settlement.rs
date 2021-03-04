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

    pub fn matched(order: OrderCreation, executed_amount: U256) -> Self {
        Self {
            order,
            executed_amount,
            fee_discount: 0,
        }
    }

    // The difference between the minimum you were willing to buy/maximum you were willing to sell, and what you ended up buying/selling
    pub fn surplus(&self, sell_token_price: U256, buy_token_price: U256) -> Option<U256> {
        match self.order.kind {
            model::order::OrderKind::Buy => buy_order_surplus(
                sell_token_price,
                buy_token_price,
                self.order.sell_amount,
                self.order.buy_amount,
                self.executed_amount,
            ),
            model::order::OrderKind::Sell => sell_order_surplus(
                sell_token_price,
                buy_token_price,
                self.order.sell_amount,
                self.order.buy_amount,
                self.executed_amount,
            ),
        }
    }
}

pub trait Interaction: std::fmt::Debug + Send {
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

    // For now this computes the total surplus of all EOA trades.
    pub fn objective_value(&self) -> U256 {
        // TODO: in order to achieve a fair comparison between different solutions, we need to normalize the price
        // vector as otherwise any price vector scaling by factor x would also scale the objective value by that factor
        // without affecting the validity of the solution.
        match self.trades.iter().fold(Some(U256::zero()), |acc, trade| {
            let sell_token_price = self
                .clearing_prices
                .get(&trade.order.sell_token)
                .expect("Solution with trade but without price for sell token");
            let buy_token_price = self
                .clearing_prices
                .get(&trade.order.buy_token)
                .expect("Solution with trade but without price for sell token");
            acc?.checked_add(trade.surplus(*sell_token_price, *buy_token_price)?)
        }) {
            Some(value) => value,
            None => {
                tracing::error!("Overflow computing objective value for: {:?}", self);
                U256::zero()
            }
        }
    }
}

// The difference between what you were willing to sell (executed_amount * limit_price) converted into reference token (multiplied by buy_token_price)
// and what you had to sell denominated in the reference token (executed_amount * buy_token_price)
fn buy_order_surplus(
    sell_token_price: U256,
    buy_token_price: U256,
    sell_amount_limit: U256,
    buy_amount_limit: U256,
    executed_amount: U256,
) -> Option<U256> {
    executed_amount
        .checked_mul(sell_amount_limit)?
        .checked_div(buy_amount_limit)?
        .checked_mul(sell_token_price)?
        .checked_sub(executed_amount.checked_mul(buy_token_price)?)
}

// The difference of your proceeds denominated in the reference token (executed_sell_amount * sell_token_price)
// and what you were minimally willing to receive in buy tokens (executed_sell_amount * limit_price)
// converted to amount in reference token at the effective price (multiplied by buy_token_price)
fn sell_order_surplus(
    sell_token_price: U256,
    buy_token_price: U256,
    sell_amount_limit: U256,
    buy_amount_limit: U256,
    executed_amount: U256,
) -> Option<U256> {
    executed_amount.checked_mul(sell_token_price)?.checked_sub(
        executed_amount
            .checked_mul(buy_amount_limit)?
            .checked_div(sell_amount_limit)?
            .checked_mul(buy_token_price)?,
    )
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

    #[test]
    fn test_buy_order_surplus() {
        // Two goods are worth the same (100 each). If we were willing to pay up to 60 to receive 50,
        // but ended paying the price (1) we have a surplus of 10 sell units, so a total surplus of 1000.
        assert_eq!(
            buy_order_surplus(100.into(), 100.into(), 60.into(), 50.into(), 50.into()),
            Some(1000.into())
        );

        // If our trade got only half filled, we only get half the surplus
        assert_eq!(
            buy_order_surplus(100.into(), 100.into(), 60.into(), 50.into(), 25.into()),
            Some(500.into())
        );

        // No surplus if trade is not at all filled
        assert_eq!(
            buy_order_surplus(100.into(), 100.into(), 60.into(), 50.into(), 0.into()),
            Some(0.into())
        );

        // No surplus if trade is filled at limit
        assert_eq!(
            buy_order_surplus(100.into(), 100.into(), 50.into(), 50.into(), 50.into()),
            Some(0.into())
        );

        // Arithmetic error when limit price not respected
        assert_eq!(
            buy_order_surplus(100.into(), 100.into(), 40.into(), 50.into(), 50.into()),
            None
        );

        // Sell Token worth twice as much as buy token. If we were willing to sell at parity, we will
        // have a surplus of 50% of tokens, worth 200 each.
        assert_eq!(
            buy_order_surplus(200.into(), 100.into(), 50.into(), 50.into(), 50.into()),
            Some(5000.into())
        );

        // Buy Token worth twice as much as sell token. If we were willing to sell at 3:1, we will
        // have a surplus of 20 sell tokens, worth 100 each.
        assert_eq!(
            buy_order_surplus(100.into(), 200.into(), 60.into(), 20.into(), 20.into()),
            Some(2000.into())
        );
    }

    #[test]
    fn test_sell_order_surplus() {
        // Two goods are worth the same (100 each). If we were willing to receive as little as 40,
        // but ended paying the price (1) we have a surplus of 10 bought units, so a total surplus of 1000.
        assert_eq!(
            sell_order_surplus(100.into(), 100.into(), 50.into(), 40.into(), 50.into()),
            Some(1000.into())
        );

        // If our trade got only half filled, we only get half the surplus
        assert_eq!(
            sell_order_surplus(100.into(), 100.into(), 50.into(), 40.into(), 25.into()),
            Some(500.into())
        );

        // No surplus if trade is not at all filled
        assert_eq!(
            sell_order_surplus(100.into(), 100.into(), 50.into(), 40.into(), 0.into()),
            Some(0.into())
        );

        // No surplus if trade is filled at limit
        assert_eq!(
            sell_order_surplus(100.into(), 100.into(), 50.into(), 50.into(), 50.into()),
            Some(0.into())
        );

        // Arithmetic error when limit price not respected
        assert_eq!(
            sell_order_surplus(100.into(), 100.into(), 50.into(), 60.into(), 50.into()),
            None
        );

        // Sell token worth twice as much as buy token. If we were willing to buy at parity, we will
        // have a surplus of 100% of buy tokens, worth 100 each.
        assert_eq!(
            sell_order_surplus(200.into(), 100.into(), 50.into(), 50.into(), 50.into()),
            Some(5_000.into())
        );

        // Buy Token worth twice as much as sell token. If we were willing to buy at 3:1, we will
        // have a surplus of 10 sell tokens, worth 200 each.
        assert_eq!(
            buy_order_surplus(100.into(), 200.into(), 60.into(), 20.into(), 20.into()),
            Some(2000.into())
        );
    }
}
