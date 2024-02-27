//! Settlement surplus calculation

use {
    crate::domain::{
        auction::{self, order},
        eth,
        settlement,
    },
    num::BigRational,
    number::conversions::big_rational_to_u256,
    shared::conversions::U256Ext,
    std::collections::HashMap,
};

/// Settlement surplus
///
/// Denominated in surplus tokens. Contains multiple values since settlement can
/// have multiple orders with different tokens.
pub struct Surplus(HashMap<auction::order::OrderUid, eth::Asset>);

impl Surplus {
    pub fn new(trades: &[settlement::Trade]) -> Self {
        let surplus = trades
            .iter()
            .map(|trade| {
                trade
                    .surplus()
                    .map(|surplus| (trade.order_uid, surplus))
                    .unwrap_or_else(|| {
                        tracing::warn!("surplus failed for trade {:?}", trade.order_uid);
                        (
                            trade.order_uid,
                            eth::Asset {
                                token: trade.sell.token,
                                amount: Default::default(),
                            },
                        )
                    })
            })
            .collect();
        Self(surplus)
    }

    /// Surplus denominated in the native token (ETH)
    pub fn normalized_with(
        &self,
        prices: &HashMap<eth::TokenAddress, auction::NormalizedPrice>,
    ) -> Option<NormalizedSurplus> {
        let mut surplus = eth::TokenAmount::default();
        for eth::Asset { token, amount } in self.0.values() {
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

/// Main logic for surplus calculation
pub fn trade_surplus(
    kind: order::Kind,
    executed: eth::Asset,
    sell: eth::Asset,
    buy: eth::Asset,
    prices: &settlement::ClearingPrices,
) -> Option<eth::Asset> {
    match kind {
        order::Kind::Buy => {
            // scale limit sell to support partially fillable orders
            let limit_sell = sell
                .amount
                .checked_mul(*executed.amount)?
                .checked_div(*buy.amount)?;
            // difference between limit sell and executed amount converted to sell token
            limit_sell.checked_sub(
                executed
                    .amount
                    .checked_mul(prices.buy)?
                    .checked_div(prices.sell)?,
            )
        }
        order::Kind::Sell => {
            // scale limit buy to support partially fillable orders
            let limit_buy = executed
                .amount
                .checked_mul(*buy.amount)?
                .checked_div(*sell.amount)?;
            // difference between executed amount converted to buy token and limit buy
            executed
                .amount
                .checked_mul(prices.sell)?
                .checked_div(prices.buy)?
                .checked_sub(limit_buy)
        }
    }
    .map(|surplus| match kind {
        order::Kind::Buy => eth::Asset {
            amount: surplus.into(),
            token: sell.token,
        },
        order::Kind::Sell => eth::Asset {
            amount: surplus.into(),
            token: buy.token,
        },
    })
}
