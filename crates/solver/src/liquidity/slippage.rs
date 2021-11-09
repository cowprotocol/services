//! Module defining static slippage parameters for AMM liquidiy.

use ethcontract::U256;

/// Constant maximum slippage of 10 BPS (0.1%) to use for on-chain liquidity.
pub const MAX_SLIPPAGE_BPS: u16 = 10;

/// Basis points in 100%.
const BPS_BASE: u16 = 10000;

/// Multiply an integer amount by a rational, with additional handling in case
/// of overflows.
fn slippage_for_amount(amount: U256) -> U256 {
    let p = U256::from(MAX_SLIPPAGE_BPS);
    let q = U256::from(BPS_BASE);

    // In order to prevent overflow on the multiplication when dealing with
    // very large numbers. In that case we divide first and add the computed
    // rounding error.
    let product = (amount / q) * p;
    let rounding_error = {
        let numerator = (amount % q) * p;
        // Perform a ceil division so that we round up with slippage amount
        (numerator + q - 1) / q
    };

    product + rounding_error
}

/// Reduce the specified amount by the constant slippage.
pub fn amount_minus_max_slippage(amount: U256) -> U256 {
    amount.saturating_sub(slippage_for_amount(amount))
}

/// Increase the specified amount by the constant slippage.
pub fn amount_plus_max_slippage(amount: U256) -> U256 {
    amount.saturating_add(slippage_for_amount(amount))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_out_amount_with_slippage() {
        assert_eq!(amount_minus_max_slippage(0.into()), 0.into());
        assert_eq!(amount_minus_max_slippage(100.into()), 99.into());
        assert_eq!(amount_minus_max_slippage(1000.into()), 999.into());
        assert_eq!(amount_minus_max_slippage(10000.into()), 9990.into());
        assert_eq!(amount_minus_max_slippage(10001.into()), 9990.into());
        assert_eq!(
            amount_minus_max_slippage(U256::MAX),
            U256::from_dec_str(
                "115676297148078879228147414023679219945416714680974923475418126423905216510295"
            )
            .unwrap()
        );
    }

    #[test]
    fn test_in_amount_with_slippage() {
        assert_eq!(amount_plus_max_slippage(0.into()), 0.into());
        assert_eq!(amount_plus_max_slippage(100.into()), 101.into());
        assert_eq!(amount_plus_max_slippage(1000.into()), 1001.into());
        assert_eq!(amount_plus_max_slippage(10000.into()), 10010.into());
        assert_eq!(amount_plus_max_slippage(10001.into()), 10012.into());
        assert_eq!(amount_plus_max_slippage(U256::MAX), U256::MAX);
    }
}
