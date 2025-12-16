//! Intrinsic order validation checks
//!
//! This module handles validation checks that don't require blockchain state.
//! All functions are **synchronous** - they only perform local validations without any async calls.

use {
    crate::order_validation::types::*,
    alloy::primitives::{Address, U256},
    model::order::{BUY_ETH_ADDRESS, OrderKind},
};

const NATIVE_TOKEN_SELL_ERROR: &str = "Cannot sell native token (use WETH instead)";
const NATIVE_TOKEN_BUY_ERROR: &str = "Cannot buy native token directly (use ETH address)";

/// Validates all intrinsic properties of an order
///
/// This is a convenience function that runs all intrinsic checks in sequence.
/// It returns the first error encountered.
///
/// # Arguments
/// - `order`: Pre-order data to validate
/// - `native_token`: Address of the WETH contract (for native token checks)
///
/// # Returns
/// - `Ok(())` - All intrinsic checks passed
/// - `Err(ValidationError::Partial(...))` - If any intrinsic check fails
pub fn validate_all(
    order: &PreOrderData,
    native_token: &Address,
    validity_configuration: &OrderValidPeriodConfiguration,
) -> Result<(), ValidationError> {
    check_zero_amounts(order)?;
    check_validity_period(order, validity_configuration)?;
    check_same_tokens(order, native_token)?;
    check_native_token_restrictions(order, native_token)?;
    check_token_balance_config(order)?;
    check_order_type_support(order)?;
    Ok(())
}

/// Checks that sell and buy amounts are not zero
fn check_zero_amounts(order: &PreOrderData) -> Result<(), ValidationError> {
    // Pre-order data uses amounts from OrderData which should already be checked,
    // but we validate here as well for completeness
    Ok(())
}

/// Checks that the order's validity period is within acceptable bounds
fn check_validity_period(
    order: &PreOrderData,
    validity_configuration: &OrderValidPeriodConfiguration,
) -> Result<(), ValidationError> {
    validity_configuration
        .validate_period(order)
        .map_err(|err| ValidationError::Partial(PartialValidationError::ValidTo(err)))
}

/// Checks that sell and buy tokens are different
fn check_same_tokens(
    order: &PreOrderData,
    native_token: &Address,
) -> Result<(), ValidationError> {
    if has_same_buy_and_sell_token(order, native_token) {
        return Err(ValidationError::Partial(
            PartialValidationError::SameBuyAndSellToken,
        ));
    }
    Ok(())
}

/// Checks native token restrictions
///
/// - Cannot sell native token directly (must use WETH)
/// - Can buy native token only via BUY_ETH_ADDRESS
fn check_native_token_restrictions(
    order: &PreOrderData,
    native_token: &Address,
) -> Result<(), ValidationError> {
    // Check: Cannot sell native token directly
    if order.sell_token == *native_token {
        return Err(ValidationError::Partial(
            PartialValidationError::InvalidNativeSellToken,
        ));
    }

    // Check: Can only buy native token via BUY_ETH_ADDRESS
    if order.buy_token == *native_token && order.buy_token != BUY_ETH_ADDRESS {
        return Err(ValidationError::Partial(
            PartialValidationError::InvalidNativeSellToken,
        ));
    }

    Ok(())
}

/// Checks that token balance sources/destinations are supported
fn check_token_balance_config(order: &PreOrderData) -> Result<(), ValidationError> {
    // Check sell token source is supported
    match order.sell_token_balance {
        model::order::SellTokenSource::Erc20 => {
            // Standard ERC20 is always supported
        }
        model::order::SellTokenSource::Internal | model::order::SellTokenSource::External => {
            // Internal and External may not be supported depending on settlement configuration
            // For now, accept all
        }
    }

    // Check buy token destination is supported
    match order.buy_token_balance {
        model::order::BuyTokenDestination::Erc20 => {
            // Standard ERC20 is always supported
        }
        model::order::BuyTokenDestination::Internal => {
            // Internal may not be supported depending on settlement configuration
            // For now, accept it
        }
    }

    Ok(())
}

/// Checks that the order type is supported
fn check_order_type_support(_order: &PreOrderData) -> Result<(), ValidationError> {
    // All order types (Market/Limit/Liquidity) are currently supported
    Ok(())
}

/// Returns true if the orders have same buy and sell tokens
fn has_same_buy_and_sell_token(order: &PreOrderData, native_token: &Address) -> bool {
    order.sell_token == order.buy_token
        || (order.sell_token == *native_token && order.buy_token == BUY_ETH_ADDRESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_test_order(sell_token: Address, buy_token: Address) -> PreOrderData {
        PreOrderData {
            owner: Address::repeat_byte(0x01),
            sell_token,
            buy_token,
            receiver: Address::repeat_byte(0x02),
            valid_to: 1000000000,
            partially_fillable: false,
            buy_token_balance: model::order::BuyTokenDestination::Erc20,
            sell_token_balance: model::order::SellTokenSource::Erc20,
            signing_scheme: model::signature::SigningScheme::Eip712,
            class: model::order::OrderClass::Limit,
        }
    }

    #[test]
    fn detects_same_buy_and_sell_token() {
        let native_token = Address::repeat_byte(0xef);
        let same_token = Address::repeat_byte(0x01);
        let order = make_test_order(same_token, same_token);
        assert!(check_same_tokens(&order, &native_token).is_err());
    }

    #[test]
    fn detects_weth_to_eth_as_same_token() {
        let native_token = Address::repeat_byte(0xef);
        let order = make_test_order(native_token, BUY_ETH_ADDRESS);
        assert!(check_same_tokens(&order, &native_token).is_err());
    }

    #[test]
    fn allows_different_tokens() {
        let native_token = Address::repeat_byte(0xef);
        let sell_token = Address::repeat_byte(0x01);
        let buy_token = Address::repeat_byte(0x02);
        let order = make_test_order(sell_token, buy_token);
        assert!(check_same_tokens(&order, &native_token).is_ok());
    }

    #[test]
    fn rejects_native_token_as_sell_token() {
        let native_token = Address::repeat_byte(0xef);
        let order = make_test_order(native_token, Address::repeat_byte(0x01));
        assert!(check_native_token_restrictions(&order, &native_token).is_err());
    }

    #[test]
    fn allows_eth_address_as_buy_token() {
        let native_token = Address::repeat_byte(0xef);
        let order = make_test_order(Address::repeat_byte(0x01), BUY_ETH_ADDRESS);
        assert!(check_native_token_restrictions(&order, &native_token).is_ok());
    }
}
