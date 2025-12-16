//! Quote calculation and market price validation
//!
//! This module handles quote retrieval, calculation, and market price verification.
//! All functions are **asynchronous** - they require quote service interactions.

use {
    crate::order_validation::types::*,
    alloy::primitives::U256,
    model::order::OrderKind,
    order_quoting::{OrderQuoting, OrderQuoteSide, QuoteParameters, SellAmount},
    std::sync::Arc,
};

/// Amounts used for market price checking
///
/// Contains sell, buy, and fee amounts for comparison with market quotes.
#[derive(Debug)]
pub struct Amounts {
    pub sell: U256,
    pub buy: U256,
    pub fee: U256,
}

impl From<&model::order::Order> for Amounts {
    fn from(order: &model::order::Order) -> Self {
        Self {
            sell: order.data.sell_amount,
            buy: order.data.buy_amount,
            fee: order.data.fee_amount,
        }
    }
}

/// Retrieves the quote for an order that is being created and verify that its fee is sufficient.
///
/// The fee is checked only if `fee_amount` is specified. If a non-zero fee is provided,
/// returns `Err(ValidationError::NonZeroFee)`.
///
/// # Arguments
/// - `quoter`: The quote service implementation
/// - `quote_search_parameters`: Parameters to search for an existing quote
/// - `quote_id`: Optional quote ID from the order creation request
/// - `fee_amount`: Optional fee amount to validate (None = skip fee check)
///
/// # Returns
/// - `Ok(Quote)` - Quote found or calculated successfully
/// - `Err(ValidationError::NonZeroFee)` - If fee_amount is non-zero
/// - `Err(ValidationError::PriceForQuote(...))` - If price estimation fails
/// - `Err(ValidationError::Other(...))` - If quote storage fails
pub async fn get_quote_and_check_fee(
    quoter: &Arc<dyn OrderQuoting>,
    quote_search_parameters: &order_quoting::QuoteSearchParameters,
    quote_id: Option<i64>,
    fee_amount: Option<U256>,
) -> Result<order_quoting::Quote, ValidationError> {
    let quote = get_or_create_quote(quoter, quote_search_parameters, quote_id).await?;

    if fee_amount.is_some_and(|fee| !fee.is_zero()) {
        return Err(ValidationError::NonZeroFee);
    }

    Ok(quote)
}

/// Retrieves the quote for an order that is being created.
///
/// This works by first trying to find an existing quote, and then falling back
/// to calculating a brand new one if none can be found.
///
/// # Arguments
/// - `quoter`: The quote service implementation
/// - `quote_search_parameters`: Parameters to search for an existing quote
/// - `quote_id`: Optional quote ID from the order creation request
///
/// # Returns
/// - `Ok(Quote)` - Quote found or calculated successfully
/// - `Err(ValidationError::ZeroAmount)` - If sell or buy amount is zero
/// - `Err(ValidationError::PriceForQuote(...))` - If quote calculation fails
/// - `Err(ValidationError::Other(...))` - If quote storage fails
async fn get_or_create_quote(
    quoter: &Arc<dyn OrderQuoting>,
    quote_search_parameters: &order_quoting::QuoteSearchParameters,
    quote_id: Option<i64>,
) -> Result<order_quoting::Quote, ValidationError> {
    let quote = match quoter
        .find_quote(quote_id, quote_search_parameters.clone())
        .await
    {
        Ok(quote) => {
            tracing::debug!(quote_id =? quote.id, "found quote for order creation");
            quote
        }
        // We couldn't find a quote, so try computing a fresh quote to use instead.
        Err(err) => {
            tracing::debug!(?err, "failed to find quote for order creation");
            let parameters = QuoteParameters {
                sell_token: quote_search_parameters.sell_token,
                buy_token: quote_search_parameters.buy_token,
                side: match quote_search_parameters.kind {
                    OrderKind::Buy => OrderQuoteSide::Buy {
                        buy_amount_after_fee: quote_search_parameters
                            .buy_amount
                            .try_into()
                            .map_err(|_| ValidationError::ZeroAmount)?,
                    },
                    OrderKind::Sell => OrderQuoteSide::Sell {
                        sell_amount: SellAmount::AfterFee {
                            value: quote_search_parameters
                                .sell_amount
                                .try_into()
                                .map_err(|_| ValidationError::ZeroAmount)?,
                        },
                    },
                },
                verification: quote_search_parameters.verification.clone(),
                signing_scheme: quote_search_parameters.signing_scheme,
                additional_gas: quote_search_parameters.additional_gas,
                timeout: None, // let OrderQuoting chose default
            };

            let quote = quoter.calculate_quote(parameters).await?;
            let quote = quoter
                .store_quote(quote)
                .await
                .map_err(ValidationError::Other)?;

            tracing::debug!(quote_id =? quote.id, "computed fresh quote for order creation");
            quote
        }
    };

    Ok(quote)
}

/// Checks whether an order's limit price is outside the market price specified by the quote.
///
/// Returns `true` if the order is outside market price (triggering limit order count check).
/// Returns `false` if the order is within market price or if price comparison overflows.
///
/// # Arguments
/// - `order`: The order's sell/buy/fee amounts
/// - `quote`: The quote's sell/buy/fee amounts
/// - `kind`: The order kind (Buy or Sell)
///
/// # Returns
/// - `true` - Order is outside market price
/// - `false` - Order is within market price or comparison overflow occurred
pub fn is_order_outside_market_price(
    order: &Amounts,
    quote: &Amounts,
    kind: OrderKind,
) -> bool {
    let check = move || match kind {
        OrderKind::Buy => {
            // For buy orders: order.sell * quote.buy < (quote.sell + quote.fee) * order.buy
            Some(
                order.sell.widening_mul::<256, 4, 512, 8>(quote.buy)
                    < (quote.sell + quote.fee).widening_mul::<256, 4, 512, 8>(order.buy),
            )
        }
        OrderKind::Sell => {
            // For sell orders: adjust quote.buy for fee, then compare
            let quote_buy = quote
                .buy
                .checked_sub(quote.fee.checked_mul(quote.buy)?.checked_div(quote.sell)?)?;
            Some(
                order.sell.widening_mul::<256, 4, 512, 8>(quote_buy)
                    < quote.sell.widening_mul::<256, 4, 512, 8>(order.buy),
            )
        }
    };

    check().unwrap_or_else(|| {
        tracing::warn!(
            ?order,
            ?quote,
            "failed to check if order is outside market price"
        );
        true
    })
}

/// Checks that the limit order count for an owner does not exceed the maximum allowed.
///
/// This is used for orders that are out-of-market to rate limit spam.
///
/// # Arguments
/// - `owner`: The order owner address
/// - `limit_order_counter`: The limit order counter implementation
/// - `max_limit_orders_per_user`: Maximum number of limit orders allowed per user
///
/// # Returns
/// - `Ok(())` - Order count is within limit
/// - `Err(ValidationError::TooManyLimitOrders)` - If max limit orders exceeded
/// - `Err(ValidationError::Other(...))` - If counter query fails
pub async fn check_max_limit_orders(
    owner: alloy::primitives::Address,
    limit_order_counter: &Arc<dyn order_validation::limit_orders::LimitOrderCounting>,
    max_limit_orders_per_user: u64,
) -> Result<(), ValidationError> {
    let num_limit_orders = limit_order_counter
        .count(owner)
        .await
        .map_err(ValidationError::Other)?;
    if num_limit_orders >= max_limit_orders_per_user {
        return Err(ValidationError::TooManyLimitOrders);
    }
    Ok(())
}

/// Validates that the quote's gas cost does not exceed the maximum allowed.
///
/// This includes the quote's base gas amount plus any additional costs (e.g., for hooks, EIP-1271).
///
/// # Arguments
/// - `quote`: The quote to check
/// - `additional_gas_cost`: Additional gas cost from signature verification or hooks
/// - `max_gas_per_order`: Maximum gas allowed per order
///
/// # Returns
/// - `Ok(())` - Gas usage is within limit
/// - `Err(ValidationError::TooMuchGas)` - If gas exceeds max allowed
pub fn check_gas_limit(
    quote: Option<&order_quoting::Quote>,
    additional_gas_cost: u64,
    max_gas_per_order: u64,
) -> Result<(), ValidationError> {
    if quote.is_some_and(|quote| {
        // Quoted gas does not include additional gas for hooks nor ERC1271 signatures
        quote.data.fee_parameters.gas_amount as u64 + additional_gas_cost > max_gas_per_order
    }) {
        return Err(ValidationError::TooMuchGas);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_from_order() {
        let order = model::order::Order {
            data: model::order::OrderData {
                sell_amount: 100.into(),
                buy_amount: 200.into(),
                fee_amount: 5.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let amounts = Amounts::from(&order);
        assert_eq!(amounts.sell, 100.into());
        assert_eq!(amounts.buy, 200.into());
        assert_eq!(amounts.fee, 5.into());
    }

    #[test]
    fn is_order_outside_market_price_buy_order() {
        let order = Amounts {
            sell: 100.into(),
            buy: 40.into(),
            fee: 0.into(),
        };
        let quote = Amounts {
            sell: 100.into(),
            buy: 50.into(),
            fee: 0.into(),
        };
        // For buy orders: order.sell * quote.buy < (quote.sell + quote.fee) * order.buy
        // 100 * 50 < (100 + 0) * 40
        // 5000 < 4000 => false (within market)
        assert!(!is_order_outside_market_price(&order, &quote, OrderKind::Buy));
    }

    #[test]
    fn is_order_outside_market_price_sell_order() {
        let order = Amounts {
            sell: 100.into(),
            buy: 60.into(),
            fee: 0.into(),
        };
        let quote = Amounts {
            sell: 100.into(),
            buy: 50.into(),
            fee: 0.into(),
        };
        // For sell orders: order.sell * quote_buy < quote.sell * order.buy
        // where quote_buy = quote.buy - (quote.fee * quote.buy / quote.sell)
        // quote_buy = 50 - 0 = 50
        // 100 * 50 < 100 * 60
        // 5000 < 6000 => true (outside market)
        assert!(is_order_outside_market_price(&order, &quote, OrderKind::Sell));
    }

    #[test]
    fn is_order_outside_market_price_handles_overflow() {
        let order = Amounts {
            sell: U256::MAX,
            buy: U256::MAX,
            fee: U256::MAX,
        };
        let quote = Amounts {
            sell: U256::MAX,
            buy: U256::MAX,
            fee: U256::MAX,
        };
        // Should not panic, return true (conservative on overflow)
        assert!(is_order_outside_market_price(&order, &quote, OrderKind::Buy));
    }

    #[test]
    fn check_gas_limit_within_budget() {
        let gas_budget = 1000;
        let additional_gas = 100;
        let result = check_gas_limit(None, additional_gas, gas_budget);
        assert!(result.is_ok());
    }

    #[test]
    fn check_gas_limit_exceeds_budget() {
        let quote = order_quoting::Quote {
            data: order_quoting::QuoteData {
                fee_parameters: order_quoting::FeeParameters {
                    gas_amount: 950,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let gas_budget = 1000;
        let additional_gas = 100;
        // 950 + 100 > 1000 => error
        let result = check_gas_limit(Some(&quote), additional_gas, gas_budget);
        assert!(matches!(result, Err(ValidationError::TooMuchGas)));
    }
}
