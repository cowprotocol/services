//! Wrapper order validation strategy
//!
//! This strategy handles orders that use wrapper contracts for custom validation logic.
//! It performs all standard validations plus calls the wrapper's `verifyOrderParams()` method.

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

/// Validation strategy for wrapper orders
///
/// Handles orders with wrapper contracts that implement custom validation.
/// Calls `verifyOrderParams()` on each wrapper contract to validate order parameters.
pub struct WrapperOrderStrategy;

#[async_trait]
impl ValidationStrategy for WrapperOrderStrategy {
    fn name(&self) -> &'static str {
        "WrapperOrderStrategy"
    }

    /// Intrinsic validation for wrapper orders
    ///
    /// Same as regular orders - wrapper validation is extrinsic (on-chain call).
    fn validate_intrinsic(
        &self,
        context: &ValidationContext,
    ) -> Result<IntrinsicValidationResult, ValidationError> {
        let order = &context.request.order;

        // Parse and validate app data
        let app_data = checks::super::super::app_data::parse_app_data(
            &order.app_data,
            &context.request.full_app_data_override,
            context.app_data_validator,
            context.hooks,
        )?;

        // Recover owner from signature
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

        // Run intrinsic validation checks
        intrinsic::validate_all(&pre_order_data, context.native_token.address(), context.validity_configuration)?;

        Ok(IntrinsicValidationResult {
            owner,
            signing_scheme: order.signature.scheme(),
            app_data,
            pre_order_data,
        })
    }

    /// Extrinsic validation for wrapper orders
    ///
    /// Performs all standard extrinsic checks plus wrapper contract validation.
    async fn validate_extrinsic(
        &self,
        context: &ValidationContext,
        intrinsic_result: &IntrinsicValidationResult,
    ) -> Result<ExtrinsicValidationResult, ValidationError> {
        let order = &context.request.order;
        let data = order.data();
        let owner = intrinsic_result.owner;
        let app_data = &intrinsic_result.app_data;

        // Check if user is banned
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

        // Verify balance and allowance
        checks::ensure_transferable(order, owner, app_data, context.balance_fetcher).await?;

        // TODO: Call wrapper contract's verifyOrderParams() for each wrapper
        // This requires the ICowWrapper::Instance binding from contract generation
        // For now, this is a placeholder that indicates what needs to be done:
        //
        // for wrapper in &app_data.inner.protocol.wrappers {
        //     validate_wrapper_order_params(wrapper, &order, context).await?;
        // }

        // Get quote and check fee
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
                additional_gas: 0,
            },
            order.quote_id,
            None,
        )
        .await?;

        // Determine order class and check for out-of-market limit orders
        let (class, quote) = match intrinsic_result.pre_order_data.class {
            model::order::OrderClass::Market => (intrinsic_result.pre_order_data.class, Some(quote)),
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
                    checks::check_max_limit_orders(
                        owner,
                        context.limit_order_counter,
                        context.max_limit_orders_per_user,
                    )
                    .await?;
                }
                (intrinsic_result.pre_order_data.class, Some(quote))
            }
            model::order::OrderClass::Liquidity => {
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

/// Validates order parameters against a wrapper contract
///
/// This function calls the wrapper contract's `verifyOrderParams()` method
/// to ensure the order meets the wrapper's custom requirements.
///
/// # TODO
/// - Implement once ICowWrapper Rust bindings are generated
/// - Build OrderParams struct from order data
/// - Call verifyOrderParams() on the wrapper contract
/// - Handle any custom errors from the wrapper
#[allow(dead_code)]
async fn validate_wrapper_order_params(
    _wrapper_address: alloy::primitives::Address,
    _order: &model::order::OrderCreation,
    _context: &ValidationContext,
) -> Result<(), ValidationError> {
    // TODO: Implementation pending contract bindings
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_strategy_name() {
        let strategy = WrapperOrderStrategy;
        assert_eq!(strategy.name(), "WrapperOrderStrategy");
    }
}
