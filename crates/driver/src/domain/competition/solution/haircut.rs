//! Haircut logic for conservative solution bidding.
//!
//! Applies a configurable basis points reduction to solver-reported economics
//! (executed amounts and clearing prices) to make competition bids more
//! conservative, without modifying interaction calldata.

use {
    super::trade::ClearingPrices,
    crate::domain::{competition::order, eth},
    number::u256_ext::U256Ext,
    std::collections::HashMap,
};

/// Calculate haircutted executed amount for an order based on slack.
///
/// # Algorithm
///
/// 1. Calculate `out_quote` = buy amount user receives at solver's prices
/// 2. Calculate `out_limit` = minimum buy amount required by order limit
/// 3. Compute slack: `H_max = 1 - (out_limit / out_quote)` in bps
/// 4. Apply effective haircut: `H_eff = min(haircut_bps, H_max)`
/// 5. Calculate haircutted out: `out_eff = floor(out_quote * (1 - H_eff))`
/// 6. Convert back to executed amount that yields out_eff
///
/// Returns Some(new_executed_amount) if haircut was applied, None if no slack
/// available or haircut couldn't be applied.
pub fn calculate_executed_amount(
    order: &order::Order,
    executed: order::TargetAmount,
    haircut_bps: u32,
    clearing_prices: &ClearingPrices,
) -> Option<order::TargetAmount> {
    let executed = executed.0;

    // Step 1: Calculate out_quote (what user receives at solver's prices)
    let out_quote = match order.side {
        order::Side::Sell => {
            // Sell order: user receives buy tokens
            // out = executed_sell * price_sell / price_buy (with ceil div)
            executed
                .checked_mul(clearing_prices.sell)
                .and_then(|v| v.checked_ceil_div(&clearing_prices.buy))
        }
        order::Side::Buy => {
            // Buy order: user receives the executed buy amount directly
            Some(executed)
        }
    }?;

    // Step 2: Calculate out_limit (minimum buy amount required by order limit)
    let out_limit = match order.side {
        order::Side::Sell => {
            // Minimum buy tokens = executed_sell * order.buy / order.sell
            // (order limit ratio: buy/sell tokens)
            executed
                .checked_mul(order.buy.amount.0)
                .and_then(|v| v.checked_div(order.sell.amount.0))
        }
        order::Side::Buy => {
            // For buy orders, they receive exactly what they buy
            // Limit is on the sell side (how much they pay)
            // Since we're reducing buy amount, we always respect the limit
            Some(eth::U256::ZERO)
        }
    }?;

    // Step 3: Calculate maximum feasible haircut (slack)
    if out_quote <= out_limit {
        // No slack - order is at or beyond limit already
        return None;
    }

    // H_max = (1 - out_limit / out_quote) in bps
    // = (out_quote - out_limit) * 10000 / out_quote
    let slack = out_quote.checked_sub(out_limit).expect("checked above");
    let h_max_bps = slack
        .checked_mul(eth::U256::from(10_000u32))
        .and_then(|v| v.checked_div(out_quote))
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);

    if h_max_bps == 0 {
        // Effectively no slack
        return None;
    }

    // Step 4: Apply effective haircut (clamped to max)
    let h_eff_bps = haircut_bps.min(h_max_bps);

    // Step 5: Calculate haircutted out amount
    // out_eff = floor(out_quote * (10000 - h_eff) / 10000)
    let out_eff = out_quote
        .checked_mul(eth::U256::from(10_000u32.saturating_sub(h_eff_bps)))
        .and_then(|v| v.checked_div(eth::U256::from(10_000u32)))?;

    // Ensure we never go below limit (defensive)
    let out_eff = out_eff.max(out_limit);

    // Step 6: Convert back to executed amount
    let executed_eff = match order.side {
        order::Side::Sell => {
            // Need: executed_eff * price_sell / price_buy (ceil) = out_eff
            // So: executed_eff = out_eff * price_buy / price_sell (floor)
            out_eff
                .checked_mul(clearing_prices.buy)
                .and_then(|v| v.checked_div(clearing_prices.sell))
        }
        order::Side::Buy => {
            // For buy orders, executed IS the out amount
            Some(out_eff)
        }
    }?;

    // Sanity check: new executed should be less than original
    if executed_eff >= executed {
        // Haircut didn't actually reduce anything
        return None;
    }

    Some(order::TargetAmount(executed_eff))
}

/// Apply haircut directly to clearing prices.
///
/// Used for quotes where orders don't have meaningful limits (fake auction
/// orders use `buy.amount = 1`). Directly adjusts prices by the configured
/// haircut percentage:
/// - For sell orders: decrease sell price (reducing buy amount received)
/// - For buy orders: decrease buy price (reducing buy amount received)
pub fn apply_to_clearing_prices(
    prices: &mut HashMap<eth::Address, eth::U256>,
    side: order::Side,
    sell_token: eth::Address,
    buy_token: eth::Address,
    haircut_bps: u32,
) {
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
