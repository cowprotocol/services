//! Settlement surplus calculation

use {
    crate::domain::{
        auction::{self, order},
        eth,
        settlement,
    },
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

    // /// Surplus denominated in the native token (ETH)
    // pub fn normalized_with(
    //     &self,
    //     prices: &HashMap<eth::TokenAddress, auction::NormalizedPrice>,
    // ) -> Option<NormalizedSurplus> {
    //     let mut surplus = eth::TokenAmount::default();
    //     for eth::Asset { token, amount } in self.0.values() {
    //         let price = prices.get(token).cloned()?;
    //         let amount: eth::SimpleValue<BigRational> =
    // amount.to_big_rational().into();         let normalized_surplus =
    // big_rational_to_u256(&(amount * price)).ok()?.into();         surplus +=
    // normalized_surplus;     }
    //     Some(surplus)
    // }
}

/// Normalized settlement surplus
///
/// Denominated in the native token (ETH). A single value convenient for
/// comparison of settlements.
pub type NormalizedSurplus = eth::TokenAmount; // eth::Ether?

/// Main logic for surplus calculation
pub fn trade_surplus(
    kind: order::Side,
    executed: order::TargetAmount,
    sell: eth::Asset,
    buy: eth::Asset,
    prices: &settlement::ClearingPrices,
) -> Option<eth::Asset> {
    match kind {
        order::Side::Buy => {
            // scale limit sell to support partially fillable orders
            let limit_sell = sell
                .amount
                .0
                .checked_mul(executed.0)?
                .checked_div(buy.amount.0)?;
            // difference between limit sell and executed amount converted to sell token
            limit_sell.checked_sub(
                executed
                    .0
                    .checked_mul(prices.buy)?
                    .checked_div(prices.sell)?,
            )
        }
        order::Side::Sell => {
            // scale limit buy to support partially fillable orders
            let limit_buy = executed
                .0
                .checked_mul(buy.amount.0)?
                .checked_div(sell.amount.0)?;
            // difference between executed amount converted to buy token and limit buy
            executed
                .0
                .checked_mul(prices.sell)?
                .checked_div(prices.buy)?
                .checked_sub(limit_buy)
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
