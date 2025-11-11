use {
    crate::{app_data, arguments::FeeFactor},
    chrono::{TimeZone, Utc},
    model::{
        order::OrderCreationAppData,
        quote::{OrderQuote, OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, PriceQuality},
    },
    primitive_types::U256,
    shared::{
        order_quoting::{CalculateQuoteError, OrderQuoting, Quote, QuoteParameters},
        order_validation::{
            AppDataValidationError,
            OrderValidating,
            PartialValidationError,
            PreOrderData,
        },
        price_estimation::Verification,
        trade_finding,
    },
    std::sync::Arc,
    thiserror::Error,
    tracing::instrument,
};

/// Adjusted quote amounts after applying volume fee.
struct AdjustedQuoteData {
    /// Adjusted sell amount (original for SELL orders, increased for BUY
    /// orders)
    sell_amount: U256,
    /// Adjusted buy amount (reduced for SELL orders, original for BUY orders)
    buy_amount: U256,
    /// Protocol fee in basis points (e.g., "2" for 0.02%)
    protocol_fee_bps: Option<String>,
    /// Protocol fee amount in sell token
    protocol_fee_sell_amount: Option<U256>,
}

/// A high-level interface for handling API quote requests.
pub struct QuoteHandler {
    order_validator: Arc<dyn OrderValidating>,
    optimal_quoter: Arc<dyn OrderQuoting>,
    fast_quoter: Arc<dyn OrderQuoting>,
    app_data: Arc<app_data::Registry>,
    volume_fee: Option<FeeFactor>,
}

impl QuoteHandler {
    pub fn new(
        order_validator: Arc<dyn OrderValidating>,
        quoter: Arc<dyn OrderQuoting>,
        app_data: Arc<app_data::Registry>,
        volume_fee: Option<FeeFactor>,
    ) -> Self {
        Self {
            order_validator,
            optimal_quoter: quoter.clone(),
            fast_quoter: quoter,
            app_data,
            volume_fee,
        }
    }

    pub fn with_fast_quoter(mut self, fast_quoter: Arc<dyn OrderQuoting>) -> Self {
        self.fast_quoter = fast_quoter;
        self
    }
}

impl QuoteHandler {
    #[instrument(skip_all, fields(buy_token = ?request.buy_token, sell_token = ?request.sell_token, price_quality = ?request.price_quality))]
    pub async fn calculate_quote(
        &self,
        request: &OrderQuoteRequest,
    ) -> Result<OrderQuoteResponse, OrderQuoteError> {
        tracing::debug!(?request, "calculating quote");

        let full_app_data_override = match request.app_data {
            OrderCreationAppData::Hash { hash } => self.app_data.find(&hash).await.unwrap_or(None),
            _ => None,
        };

        let app_data = self
            .order_validator
            .validate_app_data(&request.app_data, &full_app_data_override)?;

        let order = PreOrderData::from(request);
        let valid_to = order.valid_to;
        self.order_validator.partial_validate(order).await?;

        let params = QuoteParameters {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            side: request.side,
            verification: Verification {
                from: request.from,
                receiver: request.receiver.unwrap_or(request.from),
                sell_token_source: request.sell_token_balance,
                buy_token_destination: request.buy_token_balance,
                pre_interactions: trade_finding::map_interactions(&app_data.interactions.pre),
                post_interactions: trade_finding::map_interactions(&app_data.interactions.post),
            },
            signing_scheme: request.signing_scheme,
            additional_gas: app_data.inner.protocol.hooks.gas_limit(),
            timeout: request.timeout,
        };

        let quote = match request.price_quality {
            PriceQuality::Optimal | PriceQuality::Verified => {
                let quote = self.optimal_quoter.calculate_quote(params).await?;
                self.optimal_quoter
                    .store_quote(quote)
                    .await
                    .map_err(CalculateQuoteError::Other)?
            }
            PriceQuality::Fast => {
                let mut quote = self.fast_quoter.calculate_quote(params).await?;
                // We maintain an API guarantee that fast quotes always have an expiry of zero,
                // because they're not very accurate and can be considered to
                // expire immediately.
                quote.data.expiration = Utc.timestamp_millis_opt(0).unwrap();
                quote
            }
        };

        let adjusted_quote = get_adjusted_quote_data(&quote, self.volume_fee, &request.side);
        let response = OrderQuoteResponse {
            quote: OrderQuote {
                sell_token: request.sell_token,
                buy_token: request.buy_token,
                receiver: request.receiver,
                sell_amount: adjusted_quote.sell_amount,
                buy_amount: adjusted_quote.buy_amount,
                valid_to,
                app_data: match &request.app_data {
                    OrderCreationAppData::Full { full } => OrderCreationAppData::Both {
                        full: full.clone(),
                        expected: request.app_data.hash(),
                    },
                    app_data => app_data.clone(),
                },
                fee_amount: quote.fee_amount,
                kind: quote.data.kind,
                partially_fillable: false,
                sell_token_balance: request.sell_token_balance,
                buy_token_balance: request.buy_token_balance,
                signing_scheme: request.signing_scheme.into(),
            },
            from: request.from,
            expiration: quote.data.expiration,
            id: quote.id,
            verified: quote.data.verified,
            protocol_fee_bps: adjusted_quote.protocol_fee_bps,
            protocol_fee_sell_amount: adjusted_quote.protocol_fee_sell_amount,
        };

        tracing::debug!(?response, "finished computing quote");
        Ok(response)
    }
}

/// Calculates the protocol fee based on volume fee and adjusts quote amounts.
///
/// Returns `Some(AdjustedQuote)` if volume fee is provided, `None` otherwise.
fn get_adjusted_quote_data(
    quote: &Quote,
    volume_fee: Option<FeeFactor>,
    side: &OrderQuoteSide,
) -> AdjustedQuoteData {
    let Some(factor) = volume_fee else {
        return AdjustedQuoteData {
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            protocol_fee_bps: None,
            protocol_fee_sell_amount: None,
        };
    };
    // Calculate the volume (surplus token amount) to apply fee to
    // Following driver's logic in
    // crates/driver/src/domain/competition/solution/fee.rs:189-202:
    let factor_f64: f64 = factor.into();
    let (protocol_fee_in_surplus_token, adjusted_sell_amount, adjusted_buy_amount) = match side {
        OrderQuoteSide::Sell { .. } => {
            // For SELL orders, fee is calculated on buy amount
            let fee_f64 = quote.buy_amount.to_f64_lossy() * factor_f64;
            let protocol_fee = U256::from_f64_lossy(fee_f64);

            // Reduce buy amount by protocol fee
            let adjusted_buy = quote.buy_amount.saturating_sub(protocol_fee);

            (protocol_fee, quote.sell_amount, adjusted_buy)
        }
        OrderQuoteSide::Buy { .. } => {
            // For BUY orders, fee is calculated on sell amount
            let fee_f64 = quote.sell_amount.to_f64_lossy() * factor_f64;
            let protocol_fee = U256::from_f64_lossy(fee_f64);

            // Increase sell amount by protocol fee
            let adjusted_sell = quote.sell_amount.saturating_add(protocol_fee);

            (protocol_fee, adjusted_sell, quote.buy_amount)
        }
    };

    // Convert protocol fee to sell token for the response
    let protocol_fee_sell_amount = match side {
        OrderQuoteSide::Sell { .. } => {
            // Fee is in buy token, convert to sell token using price ratio
            // price = buy_amount / sell_amount
            // fee_in_sell = fee_in_buy * sell_amount / buy_amount
            if quote.buy_amount.is_zero() {
                U256::zero()
            } else {
                protocol_fee_in_surplus_token
                    .full_mul(quote.sell_amount)
                    .checked_div(quote.buy_amount.into())
                    .and_then(|result| result.try_into().ok())
                    .unwrap_or_default()
            }
        }
        OrderQuoteSide::Buy { .. } => {
            // Fee is already in sell token
            protocol_fee_in_surplus_token
        }
    };

    AdjustedQuoteData {
        sell_amount: adjusted_sell_amount,
        buy_amount: adjusted_buy_amount,
        protocol_fee_bps: Some(factor.to_bps().to_string()),
        protocol_fee_sell_amount: Some(protocol_fee_sell_amount),
    }
}

/// Result from handling a quote request.
#[derive(Debug, Error)]
pub enum OrderQuoteError {
    #[error("error validating app data: {0:?}")]
    AppData(AppDataValidationError),

    #[error("error validating order data: {0:?}")]
    Order(PartialValidationError),

    #[error("error calculating quote: {0}")]
    CalculateQuote(#[from] CalculateQuoteError),
}

impl From<AppDataValidationError> for OrderQuoteError {
    fn from(err: AppDataValidationError) -> Self {
        Self::AppData(err)
    }
}

impl From<PartialValidationError> for OrderQuoteError {
    fn from(err: PartialValidationError) -> Self {
        Self::Order(err)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::arguments::FeeFactor,
        model::quote::OrderQuoteSide,
        primitive_types::U256,
        shared::order_quoting::{Quote, QuoteData},
    };

    fn to_wei(base: u32) -> U256 {
        U256::from(base) * U256::from(10).pow(U256::from(18))
    }

    fn create_test_quote(sell_amount: U256, buy_amount: U256) -> Quote {
        Quote {
            id: None,
            data: QuoteData {
                sell_token: Default::default(),
                buy_token: Default::default(),
                quoted_sell_amount: sell_amount,
                quoted_buy_amount: buy_amount,
                fee_parameters: Default::default(),
                kind: model::order::OrderKind::Sell,
                expiration: chrono::Utc::now(),
                quote_kind: database::quotes::QuoteKind::Standard,
                solver: Default::default(),
                verified: false,
                metadata: Default::default(),
            },
            sell_amount,
            buy_amount,
            fee_amount: U256::zero(),
        }
    }

    #[test]
    fn test_volume_fee_sell_order() {
        let volume_fee = FeeFactor::try_from(0.0002).unwrap(); // 0.02% = 2 bps

        // Selling 100 tokens, expecting to buy 100 tokens
        let quote = create_test_quote(to_wei(100), to_wei(100));
        let side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::U256::try_from(to_wei(100)).unwrap(),
            },
        };

        let result = get_adjusted_quote_data(&quote, Some(volume_fee), &side);

        // For SELL orders:
        // - sell_amount stays the same
        // - buy_amount is reduced by 0.02% of original buy_amount
        // - protocol_fee_bps = "2"
        assert_eq!(result.sell_amount, to_wei(100));
        assert_eq!(result.protocol_fee_bps, Some("2".to_string()));

        // buy_amount should be reduced by 0.02%
        // Expected: 100 - (100 * 0.0002) = 100 - 0.02 = 99.98
        let expected_buy = to_wei(100) - (to_wei(100) / U256::from(5000)); // 0.02% = 1/5000
        assert_eq!(result.buy_amount, expected_buy);

        // Protocol fee in sell token should be the fee converted from buy token
        // fee_in_buy = 100 * 0.0002 = 0.02
        // fee_in_sell = 0.02 * (sell_amount / buy_amount) = 0.02 * (100/100) = 0.02
        let expected_fee = to_wei(100) / U256::from(5000);
        assert_eq!(result.protocol_fee_sell_amount, Some(expected_fee));
    }

    #[test]
    fn test_volume_fee_buy_order() {
        let volume_fee = FeeFactor::try_from(0.0002).unwrap(); // 0.02% = 2 bps

        // Buying 100 tokens, expecting to sell 100 tokens
        let quote = create_test_quote(to_wei(100), to_wei(100));
        let side = OrderQuoteSide::Buy {
            buy_amount_after_fee: number::nonzero::U256::try_from(to_wei(100)).unwrap(),
        };

        let result = get_adjusted_quote_data(&quote, Some(volume_fee), &side);

        // For BUY orders:
        // - buy_amount stays the same
        // - sell_amount is increased by 0.02% of original sell_amount
        // - protocol_fee_bps = "2"
        assert_eq!(result.buy_amount, to_wei(100));
        assert_eq!(result.protocol_fee_bps, Some("2".to_string()));

        // sell_amount should be increased by 0.02%
        // Expected: 100 + (100 * 0.0002) = 100 + 0.02 = 100.02
        let expected_sell = to_wei(100) + (to_wei(100) / U256::from(5000)); // 0.02% = 1/5000
        assert_eq!(result.sell_amount, expected_sell);

        // Protocol fee in sell token is just the fee amount
        let expected_fee = to_wei(100) / U256::from(5000);
        assert_eq!(result.protocol_fee_sell_amount, Some(expected_fee));
    }

    #[test]
    fn test_volume_fee_different_prices() {
        let volume_fee = FeeFactor::try_from(0.001).unwrap(); // 0.1% = 10 bps

        // Selling 100 tokens, expecting to buy 200 tokens (2:1 price ratio)
        let quote = create_test_quote(to_wei(100), to_wei(200));
        let side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::U256::try_from(to_wei(100)).unwrap(),
            },
        };

        let result = get_adjusted_quote_data(&quote, Some(volume_fee), &side);

        assert_eq!(result.protocol_fee_bps, Some("10".to_string()));
        assert_eq!(result.sell_amount, to_wei(100));

        // buy_amount reduced by 0.1% of 200 = 0.2 tokens
        let expected_buy = to_wei(200) - (to_wei(200) / U256::from(1000));
        assert_eq!(result.buy_amount, expected_buy);

        // fee_in_buy = 200 * 0.001 = 0.2
        // fee_in_sell = 0.2 * (100 / 200) = 0.1
        let fee_in_buy = to_wei(200) / U256::from(1000);
        let expected_fee = fee_in_buy * to_wei(100) / to_wei(200);
        assert_eq!(result.protocol_fee_sell_amount, Some(expected_fee));
    }

    #[test]
    fn test_volume_fee_basis_points_conversion() {
        let test_cases = vec![
            (0.0001, "1"), // 0.01% = 1 bps
            (0.001, "10"), // 0.1% = 10 bps
            (0.01, "100"), // 1% = 100 bps
            (0.05, "500"), // 5% = 500 bps
            (0.1, "1000"), // 10% = 1000 bps
        ];

        for (factor, expected_bps) in test_cases {
            let volume_fee = FeeFactor::try_from(factor).unwrap();

            let quote = create_test_quote(to_wei(100), to_wei(100));
            let side = OrderQuoteSide::Sell {
                sell_amount: model::quote::SellAmount::BeforeFee {
                    value: number::nonzero::U256::try_from(to_wei(100)).unwrap(),
                },
            };

            let result = get_adjusted_quote_data(&quote, Some(volume_fee), &side);

            assert_eq!(result.protocol_fee_bps, Some(expected_bps.to_string()));
        }
    }
}
