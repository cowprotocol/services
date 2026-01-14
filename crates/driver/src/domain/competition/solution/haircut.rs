//! Haircut logic for conservative solution bidding.
//!
//! Applies a configurable basis points reduction to clearing prices to make
//! competition bids more conservative, without modifying interaction calldata.

use {
    crate::domain::{competition::order, eth},
    std::collections::HashMap,
};

const MAX_BASE_POINT: u32 = 10000;

/// Apply haircut directly to clearing prices.
///
/// Adjusts prices by the configured haircut percentage to reduce the reported
/// surplus, making the bid more conservative:
/// - For sell orders: decrease sell price (reducing buy amount received)
/// - For buy orders: increase buy price (increasing sell amount paid)
///
/// # Example
///
/// With a 100 bps (1%) haircut and clearing prices `{ETH: 1000, USDC: 1}`:
///
/// - **Sell order** (selling ETH for USDC): ETH price reduced to 990. Reported
///   buy amount = `sell_amount * 990 / 1` instead of `sell_amount * 1000 / 1`,
///   so surplus appears 1% lower.
///
/// - **Buy order** (buying ETH with USDC): USDC price increased to 1.01.
///   Reported sell amount = `buy_amount * 1000 / 1.01` instead of `buy_amount *
///   1000 / 1`, so surplus appears ~1% lower.
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

    match side {
        order::Side::Sell => {
            // For sell orders: decrease sell price to reduce the amount of buy tokens
            // received. new_sell_price = sell_price * (10000 - haircut_bps) / 10000
            let Some(price) = prices.get_mut(&sell_token).filter(|p| !p.is_zero()) else {
                return;
            };
            let original_price = *price;
            if let Some(adjusted) = original_price
                .checked_mul(eth::U256::from(MAX_BASE_POINT.saturating_sub(haircut_bps)))
                .and_then(|v| v.checked_div(eth::U256::from(MAX_BASE_POINT)))
            {
                tracing::debug!(
                    haircut_bps,
                    sell_price = %original_price,
                    adjusted_sell_price = %adjusted,
                    "Applying haircut to sell order: adjusting sell price"
                );
                *price = adjusted;
            }
        }
        order::Side::Buy => {
            // For buy orders: increase buy price to increase the sell tokens paid,
            // making the bid more conservative (less surplus reported).
            // new_buy_price = buy_price * (10000 + haircut_bps) / 10000
            let Some(price) = prices.get_mut(&buy_token).filter(|p| !p.is_zero()) else {
                return;
            };
            let original_price = *price;
            if let Some(adjusted) = original_price
                .checked_mul(eth::U256::from(MAX_BASE_POINT.saturating_add(haircut_bps)))
                .and_then(|v| v.checked_div(eth::U256::from(MAX_BASE_POINT)))
            {
                tracing::debug!(
                    haircut_bps,
                    buy_price = %original_price,
                    adjusted_buy_price = %adjusted,
                    "Applying haircut to buy order: adjusting buy price"
                );
                *price = adjusted;
            }
        }
    }
}
