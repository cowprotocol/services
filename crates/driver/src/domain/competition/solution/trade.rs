use {
    super::ClearingPrices,
    crate::domain::{
        competition::{self, order},
        eth,
    },
    shared::conversions::U256Ext,
};

/// A trade which executes an order as part of this solution.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

/// A trade which fulfills an order from the auction.
#[derive(Debug)]
pub struct Fulfillment {
    pub order: competition::Order,
    /// The amount executed by this fulfillment. See
    /// [`competition::order::Partial`]. If the order is not partial, the
    /// executed amount must equal the amount from the order.
    pub executed: competition::order::TargetAmount,
}

impl Fulfillment {
    /// Calculate the settlement contract input and output amounts executed by
    /// this trade.
    pub fn execution(&self, clearing_prices: &ClearingPrices) -> Result<Execution, Error> {
        let input = self.executed.to_asset(&self.order);
        let (input_price, output_price) = {
            let sell_price = clearing_prices
                .0
                .get(&self.order.sell.token)
                .ok_or(Error::ClearingPriceMissing)?
                .to_owned();
            let buy_price = clearing_prices
                .0
                .get(&self.order.buy.token)
                .ok_or(Error::ClearingPriceMissing)?
                .to_owned();
            match self.order.side {
                order::Side::Buy => (buy_price, sell_price),
                order::Side::Sell => (sell_price, buy_price),
            }
        };
        let output = eth::Asset {
            amount: match self.order.kind {
                order::Kind::Market => input
                    .amount
                    .checked_mul(input_price)
                    .ok_or(Error::Overflow)?
                    .checked_ceil_div(&output_price)
                    .ok_or(Error::Overflow)?,
                order::Kind::Limit { .. } => todo!(),
                order::Kind::Liquidity => todo!(),
            },
            token: match self.order.side {
                order::Side::Buy => self.order.sell.token,
                order::Side::Sell => self.order.buy.token,
            },
        };
        Ok(Execution { input, output })
    }
}

/// The amounts executed by a fulfillment.
#[derive(Debug, Clone, Copy)]
pub struct Execution {
    /// The amount entering the settlement contract.
    pub input: eth::Asset,
    /// The amount exiting the settlement contract.
    pub output: eth::Asset,
}

/// A trade which adds a JIT order. See [`order::Jit`].
#[derive(Debug)]
pub struct Jit {
    pub order: order::Jit,
    pub executed: competition::order::TargetAmount,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("overflow error")]
    Overflow,
    #[error("a required clearing price was missing")]
    ClearingPriceMissing,
}
