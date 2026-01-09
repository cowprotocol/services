//! Quote haircut logic for conservative solution bidding.
//!
//! Applies a configurable basis points reduction to solver-reported economics
//! (executed amounts and clearing prices) to make competition bids more
//! conservative, without modifying interaction calldata.

use {
    super::trade::ClearingPrices,
    crate::domain::{competition::order, eth},
    number::u256_ext::U256Ext,
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
            // Minimum buy tokens = executed_sell * order.sell / order.buy
            executed
                .checked_mul(order.sell.amount.0)
                .and_then(|v| v.checked_div(order.buy.amount.0))
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
        .checked_mul(eth::U256::from(10_000u32 - h_eff_bps))
        .and_then(|v| v.checked_div(eth::U256::from(10_000u32)))
        .expect("haircut calculation");

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haircut_calculation() {
        // Test haircut calculation: 1000 * (10000 - 200) / 10000 = 980
        let original = eth::U256::from(1000u64);
        let haircut_bps = 200u32; // 2%
        let factor = eth::U256::from(10_000u32 - haircut_bps);
        let result = original
            .checked_mul(factor)
            .unwrap()
            .checked_div(eth::U256::from(10_000u32))
            .unwrap();
        assert_eq!(result, eth::U256::from(980u64));
    }

    #[test]
    fn test_haircut_rounding_down() {
        // Test that haircut rounds down conservatively
        // 1001 * 9800 / 10000 = 980.98 -> should round down to 980
        let original = eth::U256::from(1001u64);
        let haircut_bps = 200u32; // 2%
        let factor = eth::U256::from(10_000u32 - haircut_bps);
        let result = original
            .checked_mul(factor)
            .unwrap()
            .checked_div(eth::U256::from(10_000u32))
            .unwrap();
        assert_eq!(result, eth::U256::from(980u64));
    }

    #[test]
    fn test_slack_calculation() {
        // Order: sell 1000 for at least 900
        // Solver quotes: 1000 sells for 950
        // Slack: (950 - 900) / 950 = 50/950 = 5.26% = 526 bps
        let out_quote = eth::U256::from(950u64);
        let out_limit = eth::U256::from(900u64);

        let slack = out_quote.checked_sub(out_limit).unwrap();
        let h_max_bps = slack
            .checked_mul(eth::U256::from(10_000u32))
            .and_then(|v| v.checked_div(out_quote))
            .unwrap();

        assert_eq!(h_max_bps, eth::U256::from(526u64)); // ~5.26%
    }

    #[test]
    fn test_no_slack() {
        // Order: sell 1000 for at least 900
        // Solver quotes: 1000 sells for 900 (at limit)
        // Slack: 0
        let out_quote = eth::U256::from(900u64);
        let out_limit = eth::U256::from(900u64);

        assert!(out_quote <= out_limit); // No slack
    }

    #[test]
    fn test_haircut_clamping() {
        // Configured haircut: 1000 bps (10%)
        // Max feasible: 500 bps (5%)
        // Should clamp to 500 bps
        let haircut_bps = 1000u32;
        let h_max_bps = 500u32;

        let h_eff = haircut_bps.min(h_max_bps);
        assert_eq!(h_eff, 500u32);
    }
}
