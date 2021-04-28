//! Module defining static slippage parameters for AMM liquidiy.

use ethcontract::U256;

/// Constant maximum slippage of 10 BPS (0.1%) to use for on-chain liquidity.
pub const MAX_SLIPPAGE_BPS: u32 = 10;

/// Basis points in 100%.
const BPS_BASE: u32 = 10000;

/// Apply the constant slippage to the specified amount.
pub fn amount_with_max_slippage(amount: U256) -> U256 {
    // If we overflow the multiplication we are dealing with very large numbers. In that case it's fine to first divide.
    let numerator = U256::from(BPS_BASE - MAX_SLIPPAGE_BPS);
    let denominator = U256::from(BPS_BASE);
    amount
        .checked_mul(numerator)
        .map(|v| v / denominator)
        .unwrap_or_else(|| (amount / denominator) * numerator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_out_amount_with_slippage() {
        assert_eq!(amount_with_max_slippage(0.into()), 0.into());
        assert_eq!(amount_with_max_slippage(100.into()), 99.into());
        assert_eq!(amount_with_max_slippage(10000.into()), 9990.into());
        assert_eq!(
            amount_with_max_slippage(U256::MAX),
            U256::from_dec_str(
                "115676297148078879228147414023679219945416714680974923475418126423905216500370"
            )
            .unwrap()
        );
    }
}
