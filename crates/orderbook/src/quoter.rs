use {
    crate::{app_data, arguments::VolumeFeeConfig},
    alloy::primitives::{U256, U512, Uint, ruint::UintTryFrom},
    bigdecimal::{BigDecimal, FromPrimitive},
    chrono::{TimeZone, Utc},
    model::{
        order::OrderCreationAppData,
        quote::{OrderQuote, OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, PriceQuality},
    },
    shared::{
        arguments::{FeeFactor, TokenBucketFeeOverride},
        fee::VolumeFeePolicy,
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
use model::order::{OrderCreation, OrderKind};
use model::quote::{CostBreakdown, NetworkFeeCost, OrderQuoteRequestV2, OrderQuoteResponseV2, ProtocolFeeCost, SlippageInfo};

const MAX_BPS: u64 = 10_000;

/// Adjusted quote amounts after applying volume fee.
struct AdjustedQuoteData {
    /// Adjusted sell amount (original for SELL orders, increased for BUY
    /// orders)
    sell_amount: U256,
    /// Adjusted buy amount (reduced for SELL orders, original for BUY orders)
    buy_amount: U256,
    /// Protocol fee in basis points (e.g., "2" for 0.02%)
    protocol_fee_bps: Option<String>,
}

impl AdjustedQuoteData {
    pub fn unchanged(quote: &Quote) -> Self {
        AdjustedQuoteData {
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            protocol_fee_bps: None,
        }
    }
}
/// A high-level interface for handling API quote requests.
pub struct QuoteHandler {
    order_validator: Arc<dyn OrderValidating>,
    optimal_quoter: Arc<dyn OrderQuoting>,
    fast_quoter: Arc<dyn OrderQuoting>,
    app_data: Arc<app_data::Registry>,
    volume_fee: Option<VolumeFeeConfig>,
    volume_fee_policy: VolumeFeePolicy,
}

impl QuoteHandler {
    pub fn new(
        order_validator: Arc<dyn OrderValidating>,
        quoter: Arc<dyn OrderQuoting>,
        app_data: Arc<app_data::Registry>,
        volume_fee: Option<VolumeFeeConfig>,
        volume_fee_bucket_overrides: Vec<TokenBucketFeeOverride>,
        enable_sell_equals_buy_volume_fee: bool,
    ) -> Self {
        let volume_fee_policy = VolumeFeePolicy::new(
            volume_fee_bucket_overrides,
            volume_fee.as_ref().and_then(|config| config.factor),
            enable_sell_equals_buy_volume_fee,
        );
        Self {
            order_validator,
            optimal_quoter: quoter.clone(),
            fast_quoter: quoter,
            app_data,
            volume_fee,
            volume_fee_policy,
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

        let adjusted_quote = get_vol_fee_adjusted_quote_data(
            &quote,
            &request.side,
            self.volume_fee.as_ref(),
            &self.volume_fee_policy,
            request.buy_token,
            request.sell_token,
        )
        .map_err(|err| OrderQuoteError::CalculateQuote(err.into()))?;
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
                gas_amount: BigDecimal::from_f64(quote.data.fee_parameters.gas_amount).ok_or(
                    OrderQuoteError::CalculateQuote(
                        anyhow::anyhow!("gas_amount is not a valid BigDecimal").into(),
                    ),
                )?,
                gas_price: BigDecimal::from_f64(quote.data.fee_parameters.gas_price).ok_or(
                    OrderQuoteError::CalculateQuote(
                        anyhow::anyhow!("gas_price is not a valid BigDecimal").into(),
                    ),
                )?,
                sell_token_price: BigDecimal::from_f64(quote.data.fee_parameters.sell_token_price)
                    .ok_or(OrderQuoteError::CalculateQuote(
                        anyhow::anyhow!("sell_token_price is not a valid BigDecimal").into(),
                    ))?,
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
        };

        tracing::debug!(?response, "finished computing quote");
        Ok(response)
    }
}

impl QuoteHandler {
    /// Calculate detailed cost breakdown from v1 quote response
    fn calculate_cost_breakdown(v1_response: &OrderQuoteResponse) -> Result<CostBreakdown, OrderQuoteError> {
        let quote = &v1_response.quote;

        // Network fee: Convert using sell_token_price.
        // 1 sell_token = X native_token (ETH/xDAI)
        // fee_amount is in sell_token
        let network_fee = Self::calculate_network_fee(quote)?;

        // Protocol fee: Calculate from protocol_fee_bps if present
        let protocol_fee = Self::calculate_protocol_fee(v1_response)?;

        // Partner fee: Will be extracted from appData later
        // TODO: extract from appData
        let partner_fee = None;

        Ok(CostBreakdown {
            network_fee,
            partner_fee,
            protocol_fee,
        })
    }

    /// Calculate network fee in both sell and buy currency.
    fn calculate_network_fee(quote: &OrderQuote) -> Result<NetworkFeeCost, OrderQuoteError> {
        // fee_amount is always in sel_token.
        let amount_in_sell_currency = quote.fee_amount;

        // Convert to buy_token using price ratio
        // if sell_token_price = X ETH and gas_price = Y ETH/gas and gas_amount = Z gas
        // then fee in sell_token = (Y * Z) / X
        // To convert to buy_token, I'll need the exchange rate between sell and buy.

        // TODO: proper conversion, using buy_token price
        // temporarily, I'll return fee_amount for both, and fix this in later.
        let amount_in_buy_currency = quote.fee_amount;

        Ok(NetworkFeeCost {
            amount_in_sell_currency,
            amount_in_buy_currency,
        })
    }

    /// Calculate protocol fee from v1 response.
    fn calculate_protocol_fee(v1_response: &OrderQuoteResponse) -> Result<ProtocolFeeCost, OrderQuoteError> {
        let quote = &v1_response.quote;

        if let Some(fee_bps_str) = &v1_response.protocol_fee_bps {
            let bps = fee_bps_str
                .parse::<u32>()
                .map_err(|_| OrderQuoteError::CalculateQuote(anyhow::anyhow!("Invalid protocol fee bps: {}", fee_bps_str).into()))?;

            // Protocol fee is calculated on the surplus token.
            let amount = match quote.kind {
                OrderKind::Sell => {
                    // For sell orders, fee is on buy_amount.
                    U256::uint_try_from(quote.buy_amount.widening_mul(U256::from(bps as u64)) / U512::from(MAX_BPS))
                        .map_err(|_| {
                            OrderQuoteError::CalculateQuote(anyhow::anyhow!("Protocol fee calculation overflow").into())
                        })?
                }
                OrderKind::Buy => {
                    // For buy orders, fee is on sell_amount.
                    U256::uint_try_from(quote.sell_amount.widening_mul(U256::from(bps as u64)) / U512::from(MAX_BPS))
                        .map_err(|_| {
                            OrderQuoteError::CalculateQuote(anyhow::anyhow!("Protocol fee calculation overflow").into())
                        })?
                }
            };

            Ok(ProtocolFeeCost { amount, bps: bps as u64 })
        } else {
            // No protocol fee
            Ok(ProtocolFeeCost {
                amount: U256::ZERO,
                bps: 0,
            })
        }
    }

    pub async fn calculate_quote_v2(&self, request: &OrderQuoteRequestV2) -> Result<OrderQuoteResponseV2, OrderQuoteError> {
        let v1_response = self.calculate_quote(&request.base).await?;

        // calculate cost breakdown.
        let costs = Self::calculate_cost_breakdown(&v1_response)?;

        // Build the signable order.
        let mut order = OrderCreation {
            sell_token: v1_response.quote.sell_token,
            buy_token: v1_response.quote.buy_token,
            receiver: v1_response.quote.receiver,
            sell_amount: v1_response.quote.sell_amount,
            buy_amount: v1_response.quote.buy_amount,
            valid_to: v1_response.quote.valid_to,
            fee_amount: U256::ZERO, // Solver-competition model uses 0 fee signed by user.
            kind: v1_response.quote.kind,
            partially_fillable: v1_response.quote.partially_fillable,
            sell_token_balance: v1_response.quote.sell_token_balance,
            buy_token_balance: v1_response.quote.buy_token_balance,
            app_data: v1_response.quote.app_data,
            quote_id: v1_response.id,
            ..Default::default()
        };

        // Apply slippage.
        self.apply_slipage(&mut order, request.slippage_bps)?;

        // Calculate amounts breakdown
        let amounts = Self::calculate_amounts_breakdown(&v1_response, &order)?;

        Ok(OrderQuoteResponseV2 {
            quote: order,
            from: v1_response.from,
            expiration: v1_response.expiration,
            id: v1_response.id,
            verified: v1_response.verified,
            amounts,
            costs,
            slippage: SlippageInfo {
                applied_bps: request.slippage_bps,
                recommended_bps: None, // TODO: smart slippage.
            },
        })
    }
}

/// Calculates the protocol fee based on volume fee and adjusts quote
/// amounts.
fn get_vol_fee_adjusted_quote_data(
    quote: &Quote,
    side: &OrderQuoteSide,
    volume_fee: Option<&VolumeFeeConfig>,
    volume_fee_policy: &VolumeFeePolicy,
    buy_token: alloy::primitives::Address,
    sell_token: alloy::primitives::Address,
) -> anyhow::Result<AdjustedQuoteData> {
    let Some(_) = volume_fee.as_ref()
        // Only apply volume fee if effective timestamp has come
        .filter(|config| config.effective_from_timestamp.is_none_or(|ts| ts <= Utc::now()))
    else {
        return Ok(AdjustedQuoteData::unchanged(quote));
    };

    // Determine applicable fee factor considering same-token config and overrides
    let factor = volume_fee_policy.get_applicable_volume_fee_factor(buy_token, sell_token, None);

    let Some(factor) = factor else {
        return Ok(AdjustedQuoteData {
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            protocol_fee_bps: None,
        });
    };
    // Calculate the volume (surplus token amount) to apply fee to
    // Following driver's logic in
    // crates/driver/src/domain/competition/solution/fee.rs:189-202:
    let (adjusted_sell_amount, adjusted_buy_amount) = match side {
        OrderQuoteSide::Sell { .. } => {
            // For SELL orders, fee is calculated on buy amount
            let protocol_fee = U256::uint_try_from(
                quote
                    .buy_amount
                    .widening_mul(U256::from(factor.to_bps()))
                    .checked_div(U512::from(FeeFactor::MAX_BPS))
                    .ok_or_else(|| anyhow::anyhow!("volume fee calculation division by zero"))?,
            )
            .map_err(|_| anyhow::anyhow!("volume fee calculation overflow"))?;

            // Reduce buy amount by protocol fee
            let adjusted_buy = quote.buy_amount.saturating_sub(protocol_fee);

            (quote.sell_amount, adjusted_buy)
        }
        OrderQuoteSide::Buy { .. } => {
            // For BUY orders, fee is calculated on sell amount + network fee.
            // Network fee is already in sell token, so it is added to get the total volume.
            let total_sell_volume = quote.sell_amount.saturating_add(quote.fee_amount);
            let factor = U256::from(factor.to_bps());
            let volume_bps: Uint<512, 8> = total_sell_volume.widening_mul(factor);
            let protocol_fee = U256::uint_try_from(
                volume_bps
                    .checked_div(U512::from(FeeFactor::MAX_BPS))
                    .ok_or_else(|| anyhow::anyhow!("volume fee calculation division by zero"))?,
            )
            .map_err(|_| anyhow::anyhow!("volume fee calculation overflow"))?;

            // Increase sell amount by protocol fee
            let adjusted_sell = quote.sell_amount.saturating_add(protocol_fee);

            (adjusted_sell, quote.buy_amount)
        }
    };

    Ok(AdjustedQuoteData {
        sell_amount: adjusted_sell_amount,
        buy_amount: adjusted_buy_amount,
        protocol_fee_bps: Some(factor.to_bps().to_string()),
    })
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
        alloy::primitives::U256,
        model::quote::OrderQuoteSide,
        number::units::EthUnit,
        shared::{
            arguments::FeeFactor,
            fee::VolumeFeePolicy,
            order_quoting::{Quote, QuoteData},
        },
    };

    const TEST_SELL_TOKEN: alloy::primitives::Address =
        alloy::primitives::address!("0000000000000000000000000000000000000001");
    const TEST_BUY_TOKEN: alloy::primitives::Address =
        alloy::primitives::address!("0000000000000000000000000000000000000002");

    fn create_test_quote(sell_amount: U256, buy_amount: U256) -> Quote {
        Quote {
            id: None,
            data: QuoteData {
                sell_token: TEST_SELL_TOKEN,
                buy_token: TEST_BUY_TOKEN,
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
            fee_amount: U256::ZERO,
        }
    }

    #[test]
    fn test_volume_fee_sell_order() {
        let volume_fee = FeeFactor::try_from(0.0002).unwrap(); // 0.02% = 2 bps
        let volume_fee_config = VolumeFeeConfig {
            factor: Some(volume_fee),
            effective_from_timestamp: None,
        };
        let volume_fee_policy = VolumeFeePolicy::new(vec![], Some(volume_fee), false);

        // Selling 100 tokens, expecting to buy 100 tokens
        let quote = create_test_quote(100u64.eth(), 100u64.eth());
        let side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::NonZeroU256::try_from(100u64.eth()).unwrap(),
            },
        };

        let result = get_vol_fee_adjusted_quote_data(
            &quote,
            &side,
            Some(&volume_fee_config),
            &volume_fee_policy,
            TEST_BUY_TOKEN,
            TEST_SELL_TOKEN,
        )
        .unwrap();

        // For SELL orders:
        // - sell_amount stays the same
        // - buy_amount is reduced by 0.02% of original buy_amount
        // - protocol_fee_bps = "2"
        assert_eq!(result.sell_amount, 100u64.eth());
        assert_eq!(result.protocol_fee_bps, Some("2".to_string()));

        // buy_amount should be reduced by 0.02%
        // Expected: 100 - (100 * 0.0002) = 100 - 0.02 = 99.98
        let expected_buy = 100u64.eth() - (100u64.eth() / U256::from(5000)); // 0.02% = 1/5000
        assert_eq!(result.buy_amount, expected_buy);
    }

    #[test]
    fn test_volume_fee_buy_order() {
        let volume_fee = FeeFactor::try_from(0.0002).unwrap(); // 0.02% = 2 bps
        let past_timestamp = Utc::now() - chrono::Duration::minutes(1);
        let volume_fee_config = VolumeFeeConfig {
            factor: Some(volume_fee),
            // Effective date in the past to ensure fee is applied
            effective_from_timestamp: Some(past_timestamp),
        };
        let volume_fee_policy = VolumeFeePolicy::new(vec![], Some(volume_fee), false);

        // Buying 100 tokens, expecting to sell 100 tokens, with no network fee
        let quote = create_test_quote(100u64.eth(), 100u64.eth());
        let side = OrderQuoteSide::Buy {
            buy_amount_after_fee: number::nonzero::NonZeroU256::try_from(100u64.eth()).unwrap(),
        };

        let result = get_vol_fee_adjusted_quote_data(
            &quote,
            &side,
            Some(&volume_fee_config),
            &volume_fee_policy,
            TEST_BUY_TOKEN,
            TEST_SELL_TOKEN,
        )
        .unwrap();

        // For BUY orders with no network fee:
        // - buy_amount stays the same
        // - sell_amount is increased by 0.02% of original sell_amount
        // - protocol_fee_bps = "2"
        assert_eq!(result.buy_amount, 100u64.eth());
        assert_eq!(result.protocol_fee_bps, Some("2".to_string()));

        // sell_amount should be increased by 0.02% of sell_amount (no network fee)
        // Expected: 100 + (100 * 0.0002) = 100 + 0.02 = 100.02
        let expected_sell = 100u64.eth() + (100u64.eth() / U256::from(5000)); // 0.02% = 1/5000
        assert_eq!(result.sell_amount, expected_sell);
    }

    #[test]
    fn test_volume_fee_buy_order_with_network_fee() {
        let volume_fee = FeeFactor::try_from(0.0002).unwrap(); // 0.02% = 2 bps
        let volume_fee_config = VolumeFeeConfig {
            factor: Some(volume_fee),
            effective_from_timestamp: None,
        };
        let volume_fee_policy = VolumeFeePolicy::new(vec![], Some(volume_fee), false);

        // Buying 100 tokens, expecting to sell 100 tokens, with 5 token network fee
        let mut quote = create_test_quote(100u64.eth(), 100u64.eth());
        quote.fee_amount = 5u64.eth(); // Network fee in sell token
        let side = OrderQuoteSide::Buy {
            buy_amount_after_fee: number::nonzero::NonZeroU256::try_from(100u64.eth()).unwrap(),
        };

        let result = get_vol_fee_adjusted_quote_data(
            &quote,
            &side,
            Some(&volume_fee_config),
            &volume_fee_policy,
            TEST_BUY_TOKEN,
            TEST_SELL_TOKEN,
        )
        .unwrap();

        // For BUY orders with network fee:
        // - buy_amount stays the same
        // - protocol fee is calculated on (sell_amount + network_fee)
        // - sell_amount is increased by protocol fee
        assert_eq!(result.buy_amount, 100u64.eth());
        assert_eq!(result.protocol_fee_bps, Some("2".to_string()));

        // Total volume = sell_amount + network_fee = 100 + 5 = 105
        // Protocol fee = 105 * 0.0002 = 0.021
        // sell_amount should be increased by protocol fee
        // Expected: 100 + 0.021 = 100.021
        let total_volume = 100u64.eth() + 5u64.eth(); // 105
        let expected_protocol_fee = total_volume / U256::from(5000); // 0.021
        let expected_sell = 100u64.eth() + expected_protocol_fee; // 100.021
        assert_eq!(result.sell_amount, expected_sell);
    }

    #[test]
    fn test_volume_fee_different_prices() {
        let volume_fee = FeeFactor::try_from(0.001).unwrap(); // 0.1% = 10 bps
        let volume_fee_config = VolumeFeeConfig {
            factor: Some(volume_fee),
            effective_from_timestamp: None,
        };
        let volume_fee_policy = VolumeFeePolicy::new(vec![], Some(volume_fee), false);

        // Selling 100 tokens, expecting to buy 200 tokens (2:1 price ratio)
        let quote = create_test_quote(100u64.eth(), 200u64.eth());
        let side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::NonZeroU256::try_from(100u64.eth()).unwrap(),
            },
        };

        let result = get_vol_fee_adjusted_quote_data(
            &quote,
            &side,
            Some(&volume_fee_config),
            &volume_fee_policy,
            TEST_BUY_TOKEN,
            TEST_SELL_TOKEN,
        )
        .unwrap();

        assert_eq!(result.protocol_fee_bps, Some("10".to_string()));
        assert_eq!(result.sell_amount, 100u64.eth());

        // buy_amount reduced by 0.1% of 200 = 0.2 tokens
        let expected_buy = 200u64.eth() - (200u64.eth() / U256::from(1000));
        assert_eq!(result.buy_amount, expected_buy);
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
            let volume_fee_config = VolumeFeeConfig {
                factor: Some(volume_fee),
                effective_from_timestamp: None,
            };
            let volume_fee_policy = VolumeFeePolicy::new(vec![], Some(volume_fee), false);

            let quote = create_test_quote(100u64.eth(), 100u64.eth());
            let side = OrderQuoteSide::Sell {
                sell_amount: model::quote::SellAmount::BeforeFee {
                    value: number::nonzero::NonZeroU256::try_from(100u64.eth()).unwrap(),
                },
            };

            let result = get_vol_fee_adjusted_quote_data(
                &quote,
                &side,
                Some(&volume_fee_config),
                &volume_fee_policy,
                TEST_BUY_TOKEN,
                TEST_SELL_TOKEN,
            )
            .unwrap();

            assert_eq!(result.protocol_fee_bps, Some(expected_bps.to_string()));
        }
    }

    #[test]
    fn test_ignore_volume_fees_before_effective_date() {
        let volume_fee = FeeFactor::try_from(0.001).unwrap(); // 0.1% = 10 bps
        let future_timestamp = Utc::now() + chrono::Duration::days(1);
        let volume_fee_config = VolumeFeeConfig {
            factor: Some(volume_fee),
            effective_from_timestamp: Some(future_timestamp),
        };
        let volume_fee_policy = VolumeFeePolicy::new(vec![], Some(volume_fee), false);

        // Selling 100 tokens, expecting to buy 100 tokens
        let quote = create_test_quote(100u64.eth(), 100u64.eth());
        let side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::NonZeroU256::try_from(100u64.eth()).unwrap(),
            },
        };

        let result = get_vol_fee_adjusted_quote_data(
            &quote,
            &side,
            Some(&volume_fee_config),
            &volume_fee_policy,
            TEST_BUY_TOKEN,
            TEST_SELL_TOKEN,
        )
        .unwrap();

        // Since the effective date is in the future, no volume fee should be applied
        assert_eq!(result.sell_amount, 100u64.eth());
        assert_eq!(result.buy_amount, 100u64.eth());
        assert_eq!(result.protocol_fee_bps, None);
    }
}
