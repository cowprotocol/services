//! Shared mathematical functions for slippage and minimum surplus calculations.

use {
    crate::{
        domain::{auction, eth},
        util::conv,
    },
    alloy::primitives::U256,
    bigdecimal::{BigDecimal, Zero},
    num::{BigUint, Integer},
};

/// Computes the absolute tolerance amount from a relative factor.
pub fn compute_absolute_tolerance(amount: U256, factor: &BigDecimal) -> U256 {
    let amount = conv::u256_to_biguint(&amount);
    let (int, exp) = factor.as_bigint_and_exponent();

    let numer = amount * int.to_biguint().expect("positive by construction");
    let denom = BigUint::from(10_u8).pow(exp.unsigned_abs().try_into().unwrap_or(u32::MAX));

    let abs = numer.div_ceil(&denom);
    conv::biguint_to_u256(&abs).unwrap_or(U256::MAX)
}

/// Converts an absolute slippage value to a relative slippage value based on
/// the amount and asset's price.
pub fn absolute_to_relative(
    absolute: Option<eth::Ether>,
    asset: &eth::Asset,
    tokens: &auction::Tokens,
) -> Option<BigDecimal> {
    let price = tokens.reference_price(&asset.token)?;
    if price.0.0.is_zero() {
        return None;
    }

    // Convert absolute slippage and asset value to ETH using BigDecimal
    let absolute = conv::ether_to_decimal(&absolute?);
    let amount_in_token = conv::ether_to_decimal(&eth::Ether(asset.amount));
    let price_in_eth = conv::ether_to_decimal(&price.0);

    // Calculate asset value in ETH: amount * price
    let amount_in_eth = amount_in_token * price_in_eth;

    // Check if amount_in_eth is zero to prevent division by zero
    if amount_in_eth.is_zero() {
        return None;
    }

    // Calculate absolute as relative: absolute_eth / asset_value_in_eth
    let absolute_as_relative = absolute / amount_in_eth;
    Some(absolute_as_relative)
}
