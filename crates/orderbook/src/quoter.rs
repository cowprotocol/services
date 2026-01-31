use {
    crate::{app_data, arguments::VolumeFeeConfig},
    ::app_data::{FeePolicy, PartnerFees},
    alloy::primitives::{U256, U512, Uint, ruint::UintTryFrom},
    bigdecimal::{BigDecimal, FromPrimitive},
    chrono::{TimeZone, Utc},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{
            CostBreakdown,
            NetworkFeeCost,
            OrderQuote,
            OrderQuoteRequest,
            OrderQuoteRequestV2,
            OrderQuoteResponse,
            OrderQuoteSide,
            OrderQuoteV2,
            PartnerFeeCost,
            PriceQuality,
            ProtocolFeeCost,
            QuoteBreakdown,
            SigningMethod,
            SlippageInfo,
        },
    },
    shared::{
        arguments::{FeeFactor, TokenBucketFeeOverride},
        fee::VolumeFeePolicy,
        order_quoting::{CalculateQuoteError, OrderQuoting, Quote, QuoteParameters},
        order_validation::{
            AppDataValidationError,
            OrderAppData,
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
        let (response, _quote) = self.calculate_quote_internal(request).await?;
        Ok(response)
    }

    #[instrument(skip_all, fields(buy_token = ?request.buy_token, sell_token = ?request.sell_token, price_quality = ?request.price_quality))]
    async fn calculate_quote_internal(
        &self,
        request: &OrderQuoteRequest,
    ) -> Result<(OrderQuoteResponse, Quote), OrderQuoteError> {
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
        Ok((response, quote))
    }
}

impl QuoteHandler {
    /// Calculate detailed cost breakdown from v1 quote response
    fn calculate_cost_breakdown(
        v1_response: &OrderQuoteResponse,
        validated_app_data: &OrderAppData,
        unadjusted_quote: &Quote,
    ) -> Result<CostBreakdown, OrderQuoteError> {
        let quote = &v1_response.quote;

        // Network fee: Convert using sell_token_price.
        // 1 sell_token = X native_token (ETH/xDAI)
        // fee_amount is in sell_token
        let network_fee = Self::calculate_network_fee(quote)?;

        // Protocol fee: Calculate from protocol_fee_bps if present
        let protocol_fee = Self::calculate_protocol_fee(v1_response)?;

        // Partner fee: Extract from validated appData.
        let partner_fee = Self::extract_partner_fee(
            &validated_app_data.inner.protocol.partner_fee,
            unadjusted_quote,
        )?;

        Ok(CostBreakdown {
            network_fee,
            partner_fee,
            protocol_fee,
        })
    }

    /// Calculate network fee in both sell and buy currency.
    fn calculate_network_fee(quote: &OrderQuote) -> Result<NetworkFeeCost, OrderQuoteError> {
        // fee_amount is always in sell_token
        let amount_in_sell_currency = quote.fee_amount;

        // Convert to buy_token using the quote's exchange rate.
        // Exchange rate: buy_amount / sell_amount = how much buy token per sell token
        // NOTE: This is an approximation as it uses the quote's exchange rate.
        let amount_in_buy_currency = if quote.sell_amount.is_zero() {
            // Can't convert if sell amount is zero
            amount_in_sell_currency
        } else {
            // fee_in_buy = fee_in_sell * (buy_amount / sell_amount)
            U256::uint_try_from(
                amount_in_sell_currency.widening_mul(quote.buy_amount)
                    / U512::from(quote.sell_amount),
            )
            .unwrap_or(amount_in_sell_currency)
        };

        Ok(NetworkFeeCost {
            amount_in_sell_currency,
            amount_in_buy_currency,
        })
    }

    /// Calculate protocol fee from v1 response.
    fn calculate_protocol_fee(
        v1_response: &OrderQuoteResponse,
    ) -> Result<ProtocolFeeCost, OrderQuoteError> {
        let quote = &v1_response.quote;

        if let Some(fee_bps_str) = &v1_response.protocol_fee_bps {
            let bps = fee_bps_str.parse::<u32>().map_err(|_| {
                OrderQuoteError::CalculateQuote(
                    anyhow::anyhow!("Invalid protocol fee bps: {}", fee_bps_str).into(),
                )
            })?;

            // Protocol fee is calculated on the surplus token.
            let amount = match quote.kind {
                OrderKind::Sell => {
                    // For sell orders, fee is on buy_amount.
                    // NOTE: buy_amount in OrderQuote is already adjusted (reduced) by protocol fee
                    // So we need to calculate it such that adjusted_buy = buy_before_fee * (1 -
                    // bps/10000) adjusted_buy / (1 - bps/10000) =
                    // buy_before_fee buy_before_fee = adjusted_buy * 10000 /
                    // (10000 - bps) protocol_fee = buy_before_fee -
                    // adjusted_buy protocol_fee = adjusted_buy * 10000 / (10000
                    // - bps) - adjusted_buy protocol_fee = adjusted_buy *
                    // (10000 / (10000 - bps) - 1) protocol_fee = adjusted_buy *
                    // (10000 - (10000 - bps)) / (10000 - bps) protocol_fee =
                    // adjusted_buy * bps / (10000 - bps)

                    let denominator = MAX_BPS.saturating_sub(bps as u64);
                    if denominator == 0 {
                        return Err(OrderQuoteError::CalculateQuote(
                            anyhow::anyhow!("Protocol fee bps too high: {}", bps).into(),
                        ));
                    }
                    U256::uint_try_from(
                        quote.buy_amount.widening_mul(U256::from(bps as u64))
                            / U512::from(denominator),
                    )
                    .map_err(|_| {
                        OrderQuoteError::CalculateQuote(
                            anyhow::anyhow!("Protocol fee calculation overflow").into(),
                        )
                    })?
                }
                OrderKind::Buy => {
                    // For buy orders, fee is on sell_amount.
                    // NOTE: sell_amount in OrderQuote is already adjusted (increased) by protocol
                    // fee adjusted_sell = sell_before_fee * (1 + bps/10000)
                    // adjusted_sell / (1 + bps/10000) = sell_before_fee
                    // sell_before_fee = adjusted_sell * 10000 / (10000 + bps)
                    // protocol_fee = adjusted_sell - sell_before_fee
                    // protocol_fee = adjusted_sell - adjusted_sell * 10000 / (10000 + bps)
                    // protocol_fee = adjusted_sell * (1 - 10000 / (10000 + bps))
                    // protocol_fee = adjusted_sell * (10000 + bps - 10000) / (10000 + bps)
                    // protocol_fee = adjusted_sell * bps / (10000 + bps)

                    let denominator = MAX_BPS.saturating_add(bps as u64);
                    U256::uint_try_from(
                        quote.sell_amount.widening_mul(U256::from(bps as u64))
                            / U512::from(denominator),
                    )
                    .map_err(|_| {
                        OrderQuoteError::CalculateQuote(
                            anyhow::anyhow!("Protocol fee calculation overflow").into(),
                        )
                    })?
                }
            };

            Ok(ProtocolFeeCost {
                amount,
                bps: bps as u64,
            })
        } else {
            // No protocol fee
            Ok(ProtocolFeeCost {
                amount: U256::ZERO,
                bps: 0,
            })
        }
    }

    pub async fn calculate_quote_v2(
        &self,
        request: &OrderQuoteRequestV2,
    ) -> Result<OrderQuoteV2, OrderQuoteError> {
        let (v1_response, unadjusted_quote) = self.calculate_quote_internal(&request.base).await?;

        // Get validated app data (already validated in calculate_quote)
        let full_app_data_override = match request.base.app_data {
            OrderCreationAppData::Hash { hash } => self.app_data.find(&hash).await.unwrap_or(None),
            _ => None,
        };

        let validated_app_data = self
            .order_validator
            .validate_app_data(&request.base.app_data, &full_app_data_override)?;

        let recommended_slippage_bps =
            Self::calculate_smart_slippage(&v1_response.quote, &request.base.side).ok(); // Dont fail if smart slippage calculation errors.

        // calculate cost breakdown.
        let costs =
            Self::calculate_cost_breakdown(&v1_response, &validated_app_data, &unadjusted_quote)?;

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
            app_data: v1_response.quote.app_data.clone(),
            quote_id: v1_response.id,
            ..Default::default()
        };

        // Apply user-provided slippage to the order.
        Self::apply_slippage(&mut order, request.slippage_bps)?;

        // Calculate amounts breakdown
        let amounts = Self::calculate_amounts_breakdown(&v1_response, &order, &unadjusted_quote)?;

        let signing_method = request
            .signing_method
            .as_ref()
            .cloned()
            .unwrap_or(SigningMethod::Eip712);

        Ok(OrderQuoteV2 {
            quote: order,
            from: v1_response.from,
            expiration: v1_response.expiration,
            id: v1_response.id,
            verified: v1_response.verified,
            amounts,
            costs,
            slippage: SlippageInfo {
                applied_bps: request.slippage_bps,
                recommended_bps: recommended_slippage_bps,
            },
            signing_method,
        })
    }

    /// Apply slippage protection to the order amounts.
    fn apply_slippage(order: &mut OrderCreation, slippage_bps: u32) -> Result<(), OrderQuoteError> {
        let slippage_factor = slippage_bps as u64;

        match order.kind {
            OrderKind::Sell => {
                // For sell orders: reduce buy_amount to account for slippage
                // buyAmount = buyAmount * (10000 - slippageBps) / 10000
                order.buy_amount = U256::uint_try_from(
                    order
                        .buy_amount
                        .widening_mul(U256::from(MAX_BPS.saturating_sub(slippage_factor)))
                        / U512::from(MAX_BPS),
                )
                .map_err(|_| {
                    OrderQuoteError::CalculateQuote(
                        anyhow::anyhow!("Slippage calculation overflow for sell order").into(),
                    )
                })?;
            }
            OrderKind::Buy => {
                // For buy orders: increase sell_amount to account for slippage
                // sellAmount = sellAmount * (10000 + slippageBps) / 10000
                order.sell_amount = U256::uint_try_from(
                    order
                        .sell_amount
                        .widening_mul(U256::from(MAX_BPS.saturating_add(slippage_factor)))
                        / U512::from(MAX_BPS),
                )
                .map_err(|_| {
                    OrderQuoteError::CalculateQuote(
                        anyhow::anyhow!("Slippage calculation overflow for buy order").into(),
                    )
                })?;
            }
        }

        Ok(())
    }

    /// Calculate the three amount breakdowns frontends need to display.
    fn calculate_amounts_breakdown(
        v1_response: &OrderQuoteResponse,
        order_after_slippage: &OrderCreation,
        unadjusted_quote: &Quote,
    ) -> Result<QuoteBreakdown, OrderQuoteError> {
        match unadjusted_quote.data.kind {
            OrderKind::Sell => {
                // For SELL orders (selling exact amount, buying at least minimum)
                let before_all_fees = unadjusted_quote.buy_amount;
                let after_network_costs = v1_response.quote.buy_amount;
                let after_slippage = order_after_slippage.buy_amount;

                Ok(QuoteBreakdown {
                    before_all_fees,
                    after_network_costs,
                    after_slippage,
                })
            }
            OrderKind::Buy => {
                // For BUY orders (buying exact amount, selling at most maximum)
                let before_all_fees = unadjusted_quote.sell_amount;
                let after_network_costs = v1_response.quote.sell_amount;
                let after_slippage = order_after_slippage.sell_amount;

                Ok(QuoteBreakdown {
                    before_all_fees,
                    after_network_costs,
                    after_slippage,
                })
            }
        }
    }

    /// Calculate smart slippage recommendation based on fee and volume.
    ///
    /// Algorithm ported from Trading SDK's suggestSlippageBps:
    /// 1. Account for potential 50% fee increase (fee slippage)
    /// 2. Account for 0.5% price movement (volume slippage)
    /// 3. Combine both into total suggested slippage.
    fn calculate_smart_slippage(
        quote: &OrderQuote,
        side: &OrderQuoteSide,
    ) -> Result<u32, OrderQuoteError> {
        // Constants
        const SLIPPAGE_FEE_MULTIPLIER_PERCENT: u64 = 50;
        const SLIPPAGE_VOLUME_MULTIPLIER_PERCENT: u64 = 50;

        let fee_amount = quote.fee_amount;

        // Determine sell amounts before and after network costs.
        let (sell_amount_before_network_costs, sell_amount_after_network_costs, is_sell) =
            match side {
                OrderQuoteSide::Sell { .. } => {
                    // For sell orders: user specifies exact sell amount
                    let before = quote.sell_amount;
                    let after = quote.sell_amount; // Fee is already in quote.fee_amount
                    (before, after, true)
                }
                OrderQuoteSide::Buy { .. } => {
                    // For buy orders: sell amount includes fee
                    let before = quote.sell_amount.saturating_sub(fee_amount);
                    let after = quote.sell_amount;
                    (before, after, false)
                }
            };

        // 1. Calculate slippage from fee (allow fee to increase by 50%)
        let slippage_from_fee =
            Self::suggest_slippage_from_fee(fee_amount, SLIPPAGE_FEE_MULTIPLIER_PERCENT);

        // 2. Calculate slippage from volume (0.5% price movement)
        let slippage_from_volume = Self::suggest_slippage_from_volume(
            sell_amount_before_network_costs,
            sell_amount_after_network_costs,
            is_sell,
            SLIPPAGE_VOLUME_MULTIPLIER_PERCENT,
        );

        // 3. Total slippage is the sum of both components.
        let total_slippage_amount = slippage_from_fee.saturating_add(slippage_from_volume);

        // 4. Convert absolute slippage amount to percentage (BPS)
        let slippage_bps = Self::calculate_slippage_bps(
            sell_amount_before_network_costs,
            sell_amount_after_network_costs,
            is_sell,
            total_slippage_amount,
        )?;

        // Clamp to reasonable bounds: min 10 bps (0.1%), max 10000 bps (100%)
        Ok(slippage_bps.clamp(10, MAX_BPS as u32))
    }

    /// Calculate slippage from fee increase.
    /// Returns absolute slippage amount in sell token.
    ///
    /// Formula: `feeAmount * (multiplyingFactorPercent / 100)`
    ///
    /// Example: `fee=100, factor=50% -> slippage = 50`
    fn suggest_slippage_from_fee(fee_amount: U256, multiplying_factor_percent: u64) -> U256 {
        // Apply percentage: fee_amount * (factor / 100)
        U256::uint_try_from(
            fee_amount.widening_mul(U256::from(multiplying_factor_percent)) / U512::from(100u64),
        )
        .unwrap_or(U256::ZERO)
    }

    /// Calculate slippage from volume/price movement.
    /// Returns absolute slippage amount in sell token.
    ///
    /// Formula: `sellAmount * (slippagePercentBps / 10000)`
    ///
    /// Example: `sellAmount=10000, slippage=40 bps (0.5%) -> slippage=5`
    fn suggest_slippage_from_volume(
        sell_amount_before_network_cost: U256,
        sell_amount_after_network_cost: U256,
        is_sell: bool,
        slippage_percent_bps: u64,
    ) -> U256 {
        // For sell orders: use amount after network costs.
        // For buy orders: use amount before network costs.
        let sell_amount = if is_sell {
            sell_amount_after_network_cost
        } else {
            sell_amount_before_network_cost
        };

        if sell_amount.is_zero() {
            return U256::ZERO;
        }

        // Apply slippage percentage: sellAmount * (bps / 10000)
        U256::uint_try_from(
            sell_amount.widening_mul(U256::from(slippage_percent_bps)) / U512::from(MAX_BPS),
        )
        .unwrap_or(U256::ZERO)
    }

    /// Convert absolute slippage amount to basis points (BPS)
    ///
    /// This uses high-precision arithmetic (1e6 scale) to avoid rounding
    /// errors.
    fn calculate_slippage_bps(
        sell_amount_before_network_cost: U256,
        sell_amount_after_network_cost: U256,
        is_sell: bool,
        slippage_amount: U256,
    ) -> Result<u32, OrderQuoteError> {
        const PRECISION_SCALE: u64 = 1_000_000; // 1e6 for precision

        let sell_amount = if is_sell {
            sell_amount_after_network_cost
        } else {
            sell_amount_before_network_cost
        };

        if sell_amount.is_zero() {
            return Ok(0);
        }

        // Calculate percentage with precision
        let percentage_scaled = if is_sell {
            // For sell: 1 - (sellAmount - slippage) / sellAmount
            // = SCALE - (SCALE * (sellAmount - slippage)) / sellAMount
            let remaining = sell_amount.saturating_sub(slippage_amount);
            let fraction = U256::uint_try_from(
                U256::from(PRECISION_SCALE).widening_mul(remaining) / U512::from(sell_amount),
            )
            .unwrap_or(U256::from(PRECISION_SCALE));

            U256::from(PRECISION_SCALE).saturating_sub(fraction)
        } else {
            // For buy: ((sellAmount + slippage) / sellAmount) - 1
            // = (SCALE * (sellAmount + slippage)) / sellAmount - SCALE
            let total = sell_amount.saturating_add(slippage_amount);
            let fraction = U256::uint_try_from(
                U256::from(PRECISION_SCALE).widening_mul(total) / U512::from(sell_amount),
            )
            .unwrap_or(U256::from(PRECISION_SCALE));

            fraction.saturating_sub(U256::from(PRECISION_SCALE))
        };

        // Convert from precision scale to BPS
        let bps_u256 = U256::uint_try_from(
            percentage_scaled.widening_mul(U256::from(MAX_BPS)) / U512::from(PRECISION_SCALE),
        )
        .unwrap_or(U256::ZERO);

        // Convert U256 to u32, safely clamping to u32::MAX
        let bps = bps_u256.to::<u64>().min(u32::MAX as u64) as u32;

        Ok(bps)
    }

    /// Extract partner fee from appData if present
    fn extract_partner_fee(
        partner_fees: &PartnerFees,
        unadjusted_quote: &Quote,
    ) -> Result<Option<PartnerFeeCost>, OrderQuoteError> {
        // Get the first partner fee (most common case is single partner fee)
        let partner_fee = partner_fees.iter().next();

        if let Some(fee) = partner_fee {
            // Extract BPS from the fee policy
            let bps = match &fee.policy {
                FeePolicy::Surplus { bps, .. } => *bps,
                FeePolicy::PriceImprovement { bps, .. } => *bps,
                FeePolicy::Volume { bps } => *bps,
            };

            if bps == 0 {
                return Ok(None);
            }

            // Calculate partner fee amount
            // Discovered and used the unadjusted quote amount as the base for the partner
            // fee to avoid double-counting with protocol fees.
            let base_amount = match unadjusted_quote.data.kind {
                OrderKind::Sell => {
                    // For sell orders: calculate on the amount the user expects to buy.
                    unadjusted_quote.buy_amount
                }
                OrderKind::Buy => {
                    // For buy orders: calculate on the amount the user expects to sell.
                    unadjusted_quote.sell_amount
                }
            };

            let amount = U256::uint_try_from(
                base_amount.widening_mul(U256::from(bps)) / U512::from(MAX_BPS),
            )
            .map_err(|_| {
                OrderQuoteError::CalculateQuote(
                    anyhow::anyhow!("Partner fee calculation overflow").into(),
                )
            })?;

            Ok(Some(PartnerFeeCost { amount, bps }))
        } else {
            Ok(None)
        }
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
        ::app_data::{AppDataHash, ProtocolAppData, ValidatedAppData},
        alloy::primitives::{Address, U256},
        model::{
            order::{BuyTokenDestination, Interactions, SellTokenSource},
            quote::OrderQuoteSide,
            signature::SigningScheme,
        },
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

    #[test]
    fn test_calculate_cost_breakdown() {
        let unadjusted_quote = create_test_quote(U256::from(10000), U256::from(20000));
        let v1_response = OrderQuoteResponse {
            quote: OrderQuote {
                fee_amount: U256::from(100),
                sell_amount: U256::from(10000),
                buy_amount: U256::from(19000),
                kind: OrderKind::Sell,
                ..create_order_quote()
            },
            protocol_fee_bps: Some("2".to_string()),
            ..create_order_quote_response()
        };
        let validated_app_data = OrderAppData {
            inner: ValidatedAppData {
                hash: AppDataHash::default(),
                document: "{}".to_string(),
                protocol: ProtocolAppData {
                    partner_fee: serde_json::from_value::<PartnerFees>(serde_json::json!([
                        {
                            "volumeBps": 100,
                            "recipient": "0x0000000000000000000000000000000000000000"
                        }
                    ]))
                    .unwrap(),
                    ..Default::default()
                },
            },
            interactions: Interactions::default(),
        };

        let costs = QuoteHandler::calculate_cost_breakdown(
            &v1_response,
            &validated_app_data,
            &unadjusted_quote,
        )
        .unwrap();

        assert_eq!(costs.network_fee.amount_in_sell_currency, U256::from(100));
        assert_eq!(costs.protocol_fee.bps, 2);
        // Partner fee: 1% of 20000 = 200
        assert_eq!(costs.partner_fee.unwrap().amount, U256::from(200));
    }
    #[test]
    fn test_calculate_network_fee() {
        let quote = OrderQuote {
            fee_amount: U256::from(100),
            sell_amount: U256::from(1000),
            buy_amount: U256::from(2000),
            ..create_order_quote()
        };

        let network_fee = QuoteHandler::calculate_network_fee(&quote).unwrap();
        assert_eq!(network_fee.amount_in_sell_currency, U256::from(100));
        // 100 * (2000 / 1000) = 200
        assert_eq!(network_fee.amount_in_buy_currency, U256::from(200));
    }

    #[test]
    fn test_extract_partner_fee() {
        let mut partner_fees =
            serde_json::from_value::<PartnerFees>(serde_json::json!([])).unwrap();
        let unadjusted_quote = create_test_quote(U256::from(10000), U256::from(20000));

        // No partner fee
        let fee = QuoteHandler::extract_partner_fee(&partner_fees, &unadjusted_quote).unwrap();
        assert!(fee.is_none());

        // 1% partner fee
        partner_fees = serde_json::from_value::<PartnerFees>(serde_json::json!([
            {
                "volumeBps": 100,
                "recipient": "0x0000000000000000000000000000000000000000"
            }
        ]))
        .unwrap();

        let fee = QuoteHandler::extract_partner_fee(&partner_fees, &unadjusted_quote)
            .unwrap()
            .unwrap();
        // Sell order: 1% of unadjusted buy_amount (20000) = 200
        assert_eq!(fee.amount, U256::from(200));
        assert_eq!(fee.bps, 100);

        // Buy order
        let mut unadjusted_quote = create_test_quote(U256::from(10000), U256::from(20000));
        unadjusted_quote.data.kind = OrderKind::Buy;
        let fee = QuoteHandler::extract_partner_fee(&partner_fees, &unadjusted_quote)
            .unwrap()
            .unwrap();
        // Buy order: 1% of unadjusted sell_amount (10000) = 100
        assert_eq!(fee.amount, U256::from(100));
    }

    #[test]
    fn test_calculate_amounts_breakdown() {
        let unadjusted_quote = create_test_quote(U256::from(10000), U256::from(20000));
        let v1_response = OrderQuoteResponse {
            quote: OrderQuote {
                sell_amount: U256::from(10000),
                buy_amount: U256::from(19000), // Adjusted down by protocol fee
                kind: OrderKind::Sell,
                ..create_order_quote()
            },
            ..create_order_quote_response()
        };
        let mut order_after_slippage = OrderCreation {
            kind: OrderKind::Sell,
            buy_amount: U256::from(18000), // Further adjusted by slippage
            ..Default::default()
        };

        let breakdown = QuoteHandler::calculate_amounts_breakdown(
            &v1_response,
            &order_after_slippage,
            &unadjusted_quote,
        )
        .unwrap();

        assert_eq!(breakdown.before_all_fees, U256::from(20000));
        assert_eq!(breakdown.after_network_costs, U256::from(19000));
        assert_eq!(breakdown.after_slippage, U256::from(18000));

        // Buy order
        let mut unadjusted_quote = create_test_quote(U256::from(10000), U256::from(20000));
        unadjusted_quote.data.kind = OrderKind::Buy;
        let v1_response = OrderQuoteResponse {
            quote: OrderQuote {
                sell_amount: U256::from(11000), // Adjusted up by protocol fee
                buy_amount: U256::from(20000),
                kind: OrderKind::Buy,
                ..create_order_quote()
            },
            ..create_order_quote_response()
        };
        order_after_slippage.kind = OrderKind::Buy;
        order_after_slippage.sell_amount = U256::from(12000); // Further adjusted by slippage

        let breakdown = QuoteHandler::calculate_amounts_breakdown(
            &v1_response,
            &order_after_slippage,
            &unadjusted_quote,
        )
        .unwrap();

        assert_eq!(breakdown.before_all_fees, U256::from(10000));
        assert_eq!(breakdown.after_network_costs, U256::from(11000));
        assert_eq!(breakdown.after_slippage, U256::from(12000));
    }

    #[test]
    fn test_apply_slippage() {
        let mut order = OrderCreation {
            kind: OrderKind::Sell,
            buy_amount: U256::from(10000),
            sell_amount: U256::from(10000),
            ..Default::default()
        };

        // 1% slippage = 100 bps
        QuoteHandler::apply_slippage(&mut order, 100).unwrap();
        // Sell order: buy_amount reduced by 1%
        assert_eq!(order.buy_amount, U256::from(9900));

        let mut order = OrderCreation {
            kind: OrderKind::Buy,
            buy_amount: U256::from(10000),
            sell_amount: U256::from(10000),
            ..Default::default()
        };
        QuoteHandler::apply_slippage(&mut order, 100).unwrap();
        // Buy order: sell_amount increased by 1%
        assert_eq!(order.sell_amount, U256::from(10100));
    }

    #[test]
    fn test_calculate_protocol_fee_sell() {
        let buy_amount_after_fee = U256::from(9998); // 10000 - 2 bps (approx)
        let v1_response = OrderQuoteResponse {
            quote: OrderQuote {
                kind: OrderKind::Sell,
                buy_amount: buy_amount_after_fee,
                ..create_order_quote()
            },
            protocol_fee_bps: Some("2".to_string()),
            ..create_order_quote_response()
        };

        let protocol_fee = QuoteHandler::calculate_protocol_fee(&v1_response).unwrap();
        // protocol_fee = 9998 * 2 / (10000 - 2) = 19996 / 9998 = 2
        assert_eq!(protocol_fee.amount, U256::from(2));
        assert_eq!(protocol_fee.bps, 2);
    }

    #[test]
    fn test_calculate_protocol_fee_buy() {
        let sell_amount_after_fee = U256::from(10002); // 10000 + 2 bps (approx)
        let v1_response = OrderQuoteResponse {
            quote: OrderQuote {
                kind: OrderKind::Buy,
                sell_amount: sell_amount_after_fee,
                ..create_order_quote()
            },
            protocol_fee_bps: Some("2".to_string()),
            ..create_order_quote_response()
        };

        let protocol_fee = QuoteHandler::calculate_protocol_fee(&v1_response).unwrap();
        // protocol_fee = 10002 * 2 / (10000 + 2) = 20004 / 10002 = 2
        assert_eq!(protocol_fee.amount, U256::from(2));
    }

    fn create_order_quote() -> OrderQuote {
        OrderQuote {
            sell_token: TEST_SELL_TOKEN,
            buy_token: TEST_BUY_TOKEN,
            receiver: None,
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            valid_to: 0,
            app_data: OrderCreationAppData::default(),
            fee_amount: U256::ZERO,
            gas_amount: BigDecimal::from(0),
            gas_price: BigDecimal::from(0),
            sell_token_price: BigDecimal::from(0),
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
            signing_scheme: SigningScheme::Eip712,
        }
    }

    fn create_order_quote_response() -> OrderQuoteResponse {
        OrderQuoteResponse {
            quote: create_order_quote(),
            from: Address::ZERO,
            expiration: chrono::Utc::now(),
            id: None,
            verified: false,
            protocol_fee_bps: None,
        }
    }

    #[test]
    fn test_suggest_slippage_from_fee_basic() {
        // Fee = 100, factor = 50% -> slippage = 50
        let slippage = QuoteHandler::suggest_slippage_from_fee(U256::from(100), 50);
        assert_eq!(slippage, U256::from(50), "50% of 100 should be 50");

        // Fee = 1000, factor = 50% -> slippage = 500
        let slippage = QuoteHandler::suggest_slippage_from_fee(U256::from(1000), 50);
        assert_eq!(slippage, U256::from(500), "50% of 1000 should be 500");

        // Fee = 0, factor = 50% -> slippage = 0
        let slippage = QuoteHandler::suggest_slippage_from_fee(U256::from(0), 50);
        assert_eq!(slippage, U256::from(0), "50% of 0 should be 0");
    }

    #[test]
    fn test_suggest_slippage_from_fee_different_factors() {
        let fee = U256::from(1000);

        // 25% factor
        let slippage = QuoteHandler::suggest_slippage_from_fee(fee, 25);
        assert_eq!(slippage, U256::from(250), "25% of 1000 should be 250");

        // 100% factor
        let slippage = QuoteHandler::suggest_slippage_from_fee(fee, 100);
        assert_eq!(slippage, U256::from(1000), "100% of 1000 should be 1000");

        // 0% factor
        let slippage = QuoteHandler::suggest_slippage_from_fee(fee, 0);
        assert_eq!(slippage, U256::from(0), "0% of 1000 should be 0");
    }

    #[test]
    fn test_suggest_slippage_from_volume_sell_order() {
        // Sell order: use amount after network costs
        // 10000 tokens, 50 bps (0.5%) slippage
        let slippage = QuoteHandler::suggest_slippage_from_volume(
            U256::from(10000), // before
            U256::from(10000), // after
            true,              // is_sell
            50,                // slippage_bps (0.5%)
        );
        // 10000 * 50 / 10000 = 50
        assert_eq!(slippage, U256::from(50), "0.5% of 10000 should be 50");
    }

    #[test]
    fn test_suggest_slippage_from_volume_buy_order() {
        // Buy order: use amount before network costs
        // 10000 tokens (after fee) - 100 tokens (before fee)
        let slippage = QuoteHandler::suggest_slippage_from_volume(
            U256::from(9900),  // before (10000 - 100 fee)
            U256::from(10000), // after
            false,             // is_buy
            50,                // slippage_bps (0.5%)
        );
        // 9900 * 50 / 10000 = 49.5 -> 49 (truncated)
        assert_eq!(slippage, U256::from(49), "0.5% of 9900 should be ~50");
    }

    #[test]
    fn test_suggest_slippage_from_volume_zero_amount() {
        // Zero amount should return zero slippage
        let slippage =
            QuoteHandler::suggest_slippage_from_volume(U256::from(0), U256::from(0), true, 50);
        assert_eq!(
            slippage,
            U256::from(0),
            "0% slippage on zero amount should be zero"
        );
    }

    #[test]
    fn test_calculate_slippage_bps_precision() {
        // Test high-precision calculation
        // 1000000 tokens, 1 token slippage
        // Expected: 1 / 1000000 = 0.0001% = 1 bps
        let bps = QuoteHandler::calculate_slippage_bps(
            U256::from(1_000_000),
            U256::from(1_000_000),
            true,
            U256::from(1),
        )
        .unwrap();
        // With 1e6 precision scale, this should give accurate result
        assert!(bps <= 2, "1 token out of 1M should be ~1 bps, got {}", bps);
    }

    #[test]
    fn test_calculate_smart_slippage_sell_order_with_fee() {
        // Sell order: selling 10000 tokens
        // Fee: 100 tokens
        // Expected calculation:
        // - Fee slippage: 100 * 50% = 50 tokens
        // - Volume slippage: 10000 * 0.5% = 50 tokens
        // - Total: 100 tokens = 1% = 100 bps
        let quote = OrderQuote {
            sell_amount: U256::from(10000),
            buy_amount: U256::from(10000), // 1:1 price
            fee_amount: U256::from(100),
            ..create_order_quote()
        };

        let side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::NonZeroU256::try_from(U256::from(10000)).unwrap(),
            },
        };

        let slippage = QuoteHandler::calculate_smart_slippage(&quote, &side).unwrap();

        // Fee slippage: 50, Volume slippage: 50, Total: 100 tokens
        // 100 / 10000 = 1% = 100 bps
        // Result is clamped to min 10 bps
        assert!(
            (100..=110).contains(&slippage),
            "Expected ~100 bps, got {}",
            slippage
        );
    }

    #[test]
    fn test_calculate_smart_slippage_sell_order_no_fee() {
        // Sell order with no fee
        // Expected: only volume slippage = 0.5% = 50 bps (clamped to min 10)
        let quote = OrderQuote {
            sell_amount: U256::from(10000),
            buy_amount: U256::from(10000),
            fee_amount: U256::from(0), // No fee
            ..create_order_quote()
        };

        let side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::NonZeroU256::try_from(U256::from(10000)).unwrap(),
            },
        };

        let slippage = QuoteHandler::calculate_smart_slippage(&quote, &side).unwrap();

        // Only volume slippage: 10000 * 0.5% = 50 tokens = 50 bps
        // Clamped to min 10 bps, but 50 > 10 so should be 50
        assert!(
            (50..=60).contains(&slippage),
            "Expected ~50 bps for no-fee sell, got {}",
            slippage
        );
    }

    #[test]
    fn test_calculate_smart_slippage_buy_order_with_fee() {
        // Buy order: buying 10000 tokens, selling ~10100 tokens (includes fee)
        // Fee: 100 tokens
        // For buy orders:
        // - sell_amount_before = 10100 - 100 = 10000
        // - sell_amount_after = 10100
        let quote = OrderQuote {
            sell_amount: U256::from(10100), // includes fee
            buy_amount: U256::from(10000),
            fee_amount: U256::from(100),
            kind: OrderKind::Buy,
            ..create_order_quote()
        };

        let side = OrderQuoteSide::Buy {
            buy_amount_after_fee: number::nonzero::NonZeroU256::try_from(U256::from(10000))
                .unwrap(),
        };

        let slippage = QuoteHandler::calculate_smart_slippage(&quote, &side).unwrap();

        // Fee slippage: 100 * 50% = 50
        // Volume slippage: 10000 * 0.5% = 50
        // Total: 100 tokens, convert to bps relative to 10000 (before fee)
        // 100 / 10000 ≈ 100 bps
        assert!(
            (100..=110).contains(&slippage),
            "Expected ~100 bps for buy with fee, got {}",
            slippage
        );
    }

    #[test]
    fn test_calculate_smart_slippage_consistency_between_orders() {
        // Same absolute amounts and fees should produce similar slippage
        // for sell and buy orders (relative to their respective bases)
        let fee = U256::from(100);
        let amount = U256::from(10000);

        // Sell order
        let sell_quote = OrderQuote {
            sell_amount: amount,
            buy_amount: amount,
            fee_amount: fee,
            kind: OrderKind::Sell,
            ..create_order_quote()
        };

        let sell_side = OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::BeforeFee {
                value: number::nonzero::NonZeroU256::try_from(amount).unwrap(),
            },
        };

        let sell_slippage =
            QuoteHandler::calculate_smart_slippage(&sell_quote, &sell_side).unwrap();

        // Buy order with equivalent amounts
        let buy_quote = OrderQuote {
            sell_amount: amount.saturating_add(fee),
            buy_amount: amount,
            fee_amount: fee,
            kind: OrderKind::Buy,
            ..create_order_quote()
        };

        let buy_side = OrderQuoteSide::Buy {
            buy_amount_after_fee: number::nonzero::NonZeroU256::try_from(amount).unwrap(),
        };

        let buy_slippage = QuoteHandler::calculate_smart_slippage(&buy_quote, &buy_side).unwrap();

        // Both should produce slippage in similar range
        // (exact values might differ due to order type specifics)
        assert!(
            (sell_slippage.cast_signed() - buy_slippage.cast_signed()).abs() < 50,
            "Sell slippage {} vs Buy slippage {} - should be similar",
            sell_slippage,
            buy_slippage
        );
    }
}
