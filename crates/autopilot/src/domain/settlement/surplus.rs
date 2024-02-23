//! Settlement surplus calculation

use {
    super::Encoded,
    crate::domain::{auction, auction::order, eth},
    num::BigRational,
    number::conversions::big_rational_to_u256,
    shared::conversions::U256Ext,
    std::collections::HashMap,
};

/// Settlement surplus
///
/// Denominated in surplus tokens. Contains multiple values since settlement can
/// have multiple orders with different tokens.
pub struct Surplus {
    surplus: HashMap<eth::TokenAddress, eth::TokenAmount>,
}

impl Surplus {
    pub fn new(settlement: &Encoded) -> Self {
        let tokens = settlement.tokens();
        let clearing_prices = settlement.clearing_prices();

        let mut surplus: HashMap<eth::TokenAddress, eth::TokenAmount> = Default::default();
        for trade in settlement.trades() {
            let clearing_prices = ClearingPrices {
                sell: clearing_prices[trade.sell_token_index],
                buy: clearing_prices[trade.buy_token_index],
            };

            let trade_surplus = trade_surplus(
                trade.flags.order_kind(),
                trade.executed,
                trade.sell_amount,
                trade.buy_amount,
                &clearing_prices,
            )
            .map(|surplus| match trade.flags.order_kind() {
                order::Kind::Buy => eth::Asset {
                    amount: surplus.into(),
                    token: tokens[trade.sell_token_index],
                },
                order::Kind::Sell => eth::Asset {
                    amount: surplus.into(),
                    token: tokens[trade.buy_token_index],
                },
            });

            match trade_surplus {
                Some(trade_surplus) => {
                    *surplus.entry(trade_surplus.token).or_default() += trade_surplus.amount;
                }
                None => tracing::warn!("surplus failed for trade {:?}", trade.order_uid),
            };
        }

        Self { surplus }
    }

    /// Surplus denominated in the native token (ETH)
    pub fn normalized_with(
        &self,
        prices: &HashMap<eth::TokenAddress, auction::NormalizedPrice>,
    ) -> Option<NormalizedSurplus> {
        let mut surplus = eth::TokenAmount::default();
        for (token, amount) in &self.surplus {
            let price = prices.get(token).cloned()?;
            let amount: eth::SimpleValue<BigRational> = amount.to_big_rational().into();
            let normalized_surplus = big_rational_to_u256(&(amount * price)).ok()?.into();
            surplus += normalized_surplus;
        }
        Some(surplus)
    }
}

/// Normalized settlement surplus
///
/// Denominated in the native token (ETH). A single value convenient for
/// comparison of settlements.
pub type NormalizedSurplus = eth::TokenAmount; // eth::Ether?

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

/// Main logic for surplus calculation
fn trade_surplus(
    kind: order::Kind,
    executed: eth::TargetAmount,
    sell_amount: eth::TokenAmount,
    buy_amount: eth::TokenAmount,
    prices: &ClearingPrices,
) -> Option<eth::TokenAmount> {
    match kind {
        order::Kind::Buy => {
            // scale limit sell to support partially fillable orders
            let limit_sell = sell_amount
                .checked_mul(*executed)?
                .checked_div(*buy_amount)?;
            // difference between limit sell and executed amount converted to sell token
            limit_sell.checked_sub(executed.checked_mul(prices.buy)?.checked_div(prices.sell)?)
        }
        order::Kind::Sell => {
            // scale limit buy to support partially fillable orders
            let limit_buy = executed
                .checked_mul(*buy_amount)?
                .checked_div(*sell_amount)?;
            // difference between executed amount converted to buy token and limit buy
            executed
                .checked_mul(prices.sell)?
                .checked_div(prices.buy)?
                .checked_sub(limit_buy)
        }
    }
    .map(Into::into)
}
