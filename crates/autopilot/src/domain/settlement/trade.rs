pub use error::Error;

use crate::{
    domain::{
        self,
        auction::{self, order},
        eth,
    },
    util::conv::U256Ext,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Trade {
    order_uid: domain::OrderUid,
    sell: eth::Asset,
    buy: eth::Asset,
    side: order::Side,
    executed: order::TargetAmount,
    prices: Prices,
}

impl Trade {
    pub fn new(
        order_uid: domain::OrderUid,
        sell: eth::Asset,
        buy: eth::Asset,
        side: order::Side,
        executed: order::TargetAmount,
        prices: Prices,
    ) -> Self {
        Self {
            order_uid,
            sell,
            buy,
            side,
            executed,
            prices,
        }
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// Denominated in NATIVE token
    pub fn native_surplus(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        let surplus = self.surplus()?;
        let price = prices
            .get(&surplus.token)
            .ok_or(Error::MissingPrice(surplus.token))?;

        Ok(price.in_eth(surplus.amount))
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// Denominated in SURPLUS token
    pub fn surplus(&self) -> Result<eth::Asset, error::Math> {
        trade_surplus(
            self.side,
            self.executed,
            self.sell,
            self.buy,
            &self.prices.custom,
        )
    }
}

#[derive(Debug)]
pub struct Prices {
    pub uniform: ClearingPrices,
    /// Adjusted uniform prices to account for fees (gas cost and protocol fees)
    pub custom: ClearingPrices,
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

fn trade_surplus(
    kind: order::Side,
    executed: order::TargetAmount,
    sell: eth::Asset,
    buy: eth::Asset,
    prices: &ClearingPrices,
) -> Result<eth::Asset, error::Math> {
    match kind {
        order::Side::Buy => {
            // scale limit sell to support partially fillable orders
            let limit_sell = sell
                .amount
                .0
                .checked_mul(executed.0)
                .ok_or(error::Math::Overflow)?
                .checked_div(buy.amount.0)
                .ok_or(error::Math::DivisionByZero)?;
            let sold = executed
                .0
                .checked_mul(prices.buy)
                .ok_or(error::Math::Overflow)?
                .checked_div(prices.sell)
                .ok_or(error::Math::DivisionByZero)?;
            // difference between limit sell and executed amount converted to sell token
            limit_sell.checked_sub(sold).ok_or(error::Math::Negative)
        }
        order::Side::Sell => {
            // scale limit buy to support partially fillable orders
            let limit_buy = executed
                .0
                .checked_mul(buy.amount.0)
                .ok_or(error::Math::Overflow)?
                .checked_div(sell.amount.0)
                .ok_or(error::Math::DivisionByZero)?;
            let bought = executed
                .0
                .checked_mul(prices.sell)
                .ok_or(error::Math::Overflow)?
                .checked_ceil_div(&prices.buy)
                .ok_or(error::Math::DivisionByZero)?;
            // difference between executed amount converted to buy token and limit buy
            bought.checked_sub(limit_buy).ok_or(error::Math::Negative)
        }
    }
    .map(|surplus| match kind {
        order::Side::Buy => eth::Asset {
            amount: surplus.into(),
            token: sell.token,
        },
        order::Side::Sell => eth::Asset {
            amount: surplus.into(),
            token: buy.token,
        },
    })
}

pub mod error {
    use crate::domain::eth;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("missing native price for token {0:?}")]
        MissingPrice(eth::TokenAddress),
        #[error(transparent)]
        Math(#[from] Math),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Math {
        #[error("overflow")]
        Overflow,
        #[error("division by zero")]
        DivisionByZero,
        #[error("negative")]
        Negative,
    }
}
