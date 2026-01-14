//! Haircut logic for conservative solution bidding.
//!
//! Applies a configurable basis points reduction to clearing prices to make
//! competition bids more conservative, without modifying interaction calldata.

use {
    crate::domain::{competition::order, eth},
    std::collections::HashMap,
};

/// Apply haircut directly to clearing prices.
///
/// Adjusts prices by the configured haircut percentage to reduce the reported
/// surplus, making the bid more conservative:
/// - For sell orders: decrease sell price (reducing buy amount received)
/// - For buy orders: increase buy price (increasing sell amount paid)
///
/// This is applied to both quotes and auction orders.
pub fn apply_to_clearing_prices(
    prices: &mut HashMap<eth::Address, eth::U256>,
    side: order::Side,
    sell_token: eth::Address,
    buy_token: eth::Address,
    haircut_bps: u32,
) {
    if haircut_bps == 0 {
        return;
    }

    let sell_price = match prices.get(&sell_token).copied() {
        Some(p) if !p.is_zero() => p,
        _ => return,
    };
    let buy_price = match prices.get(&buy_token).copied() {
        Some(p) if !p.is_zero() => p,
        _ => return,
    };

    match side {
        order::Side::Sell => {
            // For sell orders: decrease sell price to reduce the amount of buy tokens
            // received. new_sell_price = sell_price * (10000 - haircut_bps) / 10000
            if let Some(adjusted_sell_price) = sell_price
                .checked_mul(eth::U256::from(10000u32.saturating_sub(haircut_bps)))
                .and_then(|v| v.checked_div(eth::U256::from(10000u32)))
            {
                tracing::debug!(
                    haircut_bps,
                    %sell_price,
                    %adjusted_sell_price,
                    "Applying haircut to sell order: adjusting sell price"
                );
                prices.insert(sell_token, adjusted_sell_price);
            }
        }
        order::Side::Buy => {
            // For buy orders: increase buy price to increase the sell tokens paid,
            // making the bid more conservative (less surplus reported).
            // new_buy_price = buy_price * (10000 + haircut_bps) / 10000
            if let Some(adjusted_buy_price) = buy_price
                .checked_mul(eth::U256::from(10000u32.saturating_add(haircut_bps)))
                .and_then(|v| v.checked_div(eth::U256::from(10000u32)))
            {
                tracing::debug!(
                    haircut_bps,
                    %buy_price,
                    %adjusted_buy_price,
                    "Applying haircut to buy order: adjusting buy price"
                );
                prices.insert(buy_token, adjusted_buy_price);
            }
        }
    }
}
