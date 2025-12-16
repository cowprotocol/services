//! EIP-1271 smart contract signature validation strategy
//!
//! This strategy handles orders signed by smart contracts implementing the EIP-1271
//! interface. It performs all standard validations plus on-chain signature verification.

use {
    super::strategy::{
        ValidationStrategy, ValidationContext, IntrinsicValidationResult, ExtrinsicValidationResult,
    },
    crate::order_validation::{
        checks::{self, intrinsic},
        types::*,
    },
    alloy::primitives::Address,
    async_trait::async_trait,
    model::signature::Signature,
};

/// Validation strategy for EIP-1271 orders
///
/// Handles orders signed by smart contracts that implement the EIP-1271
/// standard interface. Requires on-chain signature validation.
pub struct Eip1271OrderStrategy;

#[async_trait]
impl ValidationStrategy for Eip1271OrderStrategy {
    fn name(&self) -> &'static str {
        "Eip1271OrderStrategy"
    }

    /// Intrinsic validation for EIP-1271 orders
    ///
    /// Same as regular orders since signature owner doesn't need to be recovered
    /// (it comes from the order metadata, typically msg.sender at placement time).
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

        // For EIP-1271, owner comes from the order's `from` field (order creator)
        let owner = order.from;

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

    /// Extrinsic validation for EIP-1271 orders
    ///
    /// Performs all standard extrinsic checks plus on-chain signature verification.
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

        // Validate EIP-1271 signature on-chain
        let verification_gas_limit = if let Signature::Eip1271(sig_bytes) = &order.signature {
            checks::validate_eip1271_if_needed(
                sig_bytes,
                owner,
                &context.request.domain_separator,
                &data,
                context.signature_validator,
                &app_data.interactions.pre,
                app_data.inner.protocol.flashloan.as_ref(),
                context.eip1271_skip_creation_validation,
            )
            .await?
        } else {
            return Err(ValidationError::IncompatibleSigningScheme);
        };

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
                additional_gas: verification_gas_limit,
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

        // Check gas limit (now including signature verification gas)
        checks::check_gas_limit(quote.as_ref(), verification_gas_limit, context.max_gas_per_order)?;

        Ok(ExtrinsicValidationResult {
            quote,
            class,
            verification_gas_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eip1271_strategy_name() {
        let strategy = Eip1271OrderStrategy;
        assert_eq!(strategy.name(), "Eip1271OrderStrategy");
    }
}
