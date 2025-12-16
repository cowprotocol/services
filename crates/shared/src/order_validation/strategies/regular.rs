//! Regular order validation strategy
//!
//! This strategy handles standard orders signed with ECDSA, EthSign, or PreSign.
//! It's the fallback strategy used for most orders that don't have special requirements.

use {
    super::strategy::{
        ValidationStrategy, ValidationContext, IntrinsicValidationResult, ExtrinsicValidationResult,
    },
    crate::order_validation::{
        checks::{self, intrinsic},
        types::*,
    },
    async_trait::async_trait,
};

/// Validation strategy for regular orders
///
/// Handles standard ECDSA, EthSign, and PreSign orders with full validation.
pub struct RegularOrderStrategy;

#[async_trait]
impl ValidationStrategy for RegularOrderStrategy {
    fn name(&self) -> &'static str {
        "RegularOrderStrategy"
    }

    /// Intrinsic validation for regular orders
    ///
    /// Performs synchronous validation checks:
    /// - App data parsing (JSON validation)
    /// - Signature owner recovery (cryptographic)
    /// - Basic order property validation (amounts, validity period, tokens)
    fn validate_intrinsic(
        &self,
        context: &ValidationContext,
    ) -> Result<IntrinsicValidationResult, ValidationError> {
        let order = &context.request.order;

        // Parse and validate app data (first-class operation, prerequisite)
        let app_data = checks::super::super::app_data::parse_app_data(
            &order.app_data,
            &context.request.full_app_data_override,
            context.app_data_validator,
            context.hooks,
        )?;

        // Recover owner from signature (cryptographic, sync)
        let owner = checks::recover_owner(
            order,
            &context.request.domain_separator,
            app_data.inner.protocol.appdata_signer.as_ref(),
        )?;

        // Extract pre-order data
        let pre_order_data = PreOrderData::from_order_creation(
            owner,
            &order.data(),
            order.signature.scheme(),
        );

        // Run intrinsic validation checks (all synchronous)
        intrinsic::validate_all(&pre_order_data, context.native_token.address(), context.validity_configuration)?;

        Ok(IntrinsicValidationResult {
            owner,
            signing_scheme: order.signature.scheme(),
            app_data,
            pre_order_data,
        })
    }

    /// Extrinsic validation for regular orders
    ///
    /// Performs asynchronous validation checks that require blockchain state:
    /// - Banned user check
    /// - Bad token detection
    /// - Balance and allowance verification
    /// - Quote calculation and market price validation
    /// - Gas limit enforcement
    /// - Limit order count check
    async fn validate_extrinsic(
        &self,
        context: &ValidationContext,
        intrinsic_result: &IntrinsicValidationResult,
    ) -> Result<ExtrinsicValidationResult, ValidationError> {
        let order = &context.request.order;
        let data = order.data();
        let owner = intrinsic_result.owner;
        let pre_order = &intrinsic_result.pre_order_data;
        let app_data = &intrinsic_result.app_data;

        // Check if user is banned
        // Note: This may use on-chain Chainalysis oracle, but wrapped in an async call
        if context
            .banned_users
            .is_banned(owner)
            .await
            .map_err(ValidationError::Other)?
        {
            return Err(ValidationError::Partial(PartialValidationError::Forbidden));
        }

        // Check for bad tokens
        checks::check_bad_tokens(
            data.sell_token,
            data.buy_token,
            context.bad_token_detector,
        )
        .await?;

        // Verify balance and allowance (with transfer simulation)
        checks::ensure_transferable(order, owner, app_data, context.balance_fetcher).await?;

        // Get quote and check fee (quote must be zero for regular orders during creation)
        let quote = checks::get_quote_and_check_fee(
            context.quoter,
            &order_quoting::QuoteSearchParameters {
                sell_token: data.sell_token,
                buy_token: data.buy_token,
                kind: data.kind,
                sell_amount: data.sell_amount,
                buy_amount: data.buy_amount,
                verification: order_quoting::Verification::default(),
                signing_scheme: order.signature.scheme(),
                additional_gas: 0, // Will be added after EIP-1271 check
            },
            order.quote_id,
            None, // Fee is checked later, not during intrinsic
        )
        .await?;

        // Determine order class and check for out-of-market limit orders
        let (class, quote) = match pre_order.class {
            model::order::OrderClass::Market => (pre_order.class, Some(quote)),
            model::order::OrderClass::Limit => {
                if checks::is_order_outside_market_price(
                    &checks::Amounts {
                        sell: data.sell_amount,
                        buy: data.buy_amount,
                        fee: data.fee_amount,
                    },
                    &checks::Amounts {
                        sell: quote.sell_amount,
                        buy: quote.buy_amount,
                        fee: quote.fee_amount,
                    },
                    data.kind,
                ) {
                    // Out-of-market limit order: check limit order count
                    checks::check_max_limit_orders(
                        owner,
                        context.limit_order_counter,
                        context.max_limit_orders_per_user,
                    )
                    .await?;
                }
                (pre_order.class, Some(quote))
            }
            model::order::OrderClass::Liquidity => {
                // Liquidity orders always check limit order count
                checks::check_max_limit_orders(
                    owner,
                    context.limit_order_counter,
                    context.max_limit_orders_per_user,
                )
                .await?;
                (model::order::OrderClass::Limit, None)
            }
        };

        // Check gas limit
        checks::check_gas_limit(quote.as_ref(), 0, context.max_gas_per_order)?;

        Ok(ExtrinsicValidationResult {
            quote,
            class,
            verification_gas_limit: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_strategy_name() {
        let strategy = RegularOrderStrategy;
        assert_eq!(strategy.name(), "RegularOrderStrategy");
    }
}
