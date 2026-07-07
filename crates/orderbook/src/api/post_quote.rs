use {
    super::post_order::{AppDataValidationErrorWrapper, PartialValidationErrorWrapper},
    crate::{
        api::{AppState, error, rich_error},
        quoter::OrderQuoteError,
    },
    axum::{
        Json,
        extract::State,
        response::{IntoResponse, Response},
    },
    model::quote::{OrderQuoteRequest, PriceQuality},
    reqwest::StatusCode,
    shared::order_quoting::CalculateQuoteError,
    std::sync::Arc,
};

pub async fn post_quote_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OrderQuoteRequest>,
) -> Response {
    // Record the request synchronously, before the first `.await`, so requests
    // whose future is dropped (client disconnects) are still counted. The guard
    // defaults to `cancelled` and is overwritten only once a response exists, so
    // a dropped future records a cancellation when it is dropped.
    let mut guard = QuoteRequestGuard::new(price_quality_label(&request.price_quality));

    let result = state.quotes.calculate_quote(&request).await;
    guard.finish(if result.is_ok() { "success" } else { "error" });

    result
        .map(Json)
        .inspect_err(|err| tracing::warn!(%err, ?request, "post_quote error"))
        .into_response()
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "quote")]
struct QuoteMetrics {
    /// /quote requests received, incremented at handler entry before any await
    /// so client-cancelled requests are still counted.
    #[metric(labels("price_quality"))]
    requests_started: prometheus::IntCounterVec,

    /// /quote requests that finished, by `result` (`success`, `error`, or
    /// `cancelled`). `cancelled` means the client dropped the connection before
    /// a response was produced. Invariant: started == sum(finished).
    #[metric(labels("price_quality", "result"))]
    requests_finished: prometheus::IntCounterVec,
}

/// Ties a `requests_started` increment at handler entry to a
/// `requests_finished` increment when the handler future completes or is
/// dropped. Defaults the outcome to `cancelled`; [`QuoteRequestGuard::finish`]
/// records the real outcome once a response is available.
struct QuoteRequestGuard {
    metrics: &'static QuoteMetrics,
    price_quality: &'static str,
    result: &'static str,
}

impl QuoteRequestGuard {
    fn new(price_quality: &'static str) -> Self {
        let metrics = QuoteMetrics::instance(observe::metrics::get_storage_registry()).unwrap();
        metrics
            .requests_started
            .with_label_values(&[price_quality])
            .inc();
        Self {
            metrics,
            price_quality,
            result: "cancelled",
        }
    }

    fn finish(&mut self, result: &'static str) {
        self.result = result;
    }
}

impl Drop for QuoteRequestGuard {
    fn drop(&mut self) {
        self.metrics
            .requests_finished
            .with_label_values(&[self.price_quality, self.result])
            .inc();
    }
}

fn price_quality_label(price_quality: &PriceQuality) -> &'static str {
    match price_quality {
        PriceQuality::Fast => "fast",
        PriceQuality::Optimal => "optimal",
        PriceQuality::Verified => "verified",
    }
}

impl IntoResponse for OrderQuoteError {
    fn into_response(self) -> Response {
        match self {
            OrderQuoteError::AppData(err) => AppDataValidationErrorWrapper(err).into_response(),
            OrderQuoteError::Order(err) => PartialValidationErrorWrapper(err).into_response(),
            OrderQuoteError::CalculateQuote(err) => CalculateQuoteErrorWrapper(err).into_response(),
        }
    }
}

pub struct CalculateQuoteErrorWrapper(CalculateQuoteError);
impl IntoResponse for CalculateQuoteErrorWrapper {
    fn into_response(self) -> Response {
        match self.0 {
            CalculateQuoteError::Price { source, .. } => {
                super::PriceEstimationErrorWrapper(source).into_response()
            }
            CalculateQuoteError::SellAmountDoesNotCoverFee { fee_amount } => (
                StatusCode::BAD_REQUEST,
                rich_error(
                    "SellAmountDoesNotCoverFee",
                    "The sell amount for the sell order is lower than the fee.",
                    serde_json::json!({ "fee_amount": fee_amount }),
                ),
            )
                .into_response(),
            CalculateQuoteError::QuoteNotVerified => (
                StatusCode::BAD_REQUEST,
                error(
                    "QuoteNotVerified",
                    "No quote for this trade could be verified to be accurate. Orders for this \
                     trade will likely not be executed.",
                ),
            )
                .into_response(),
            CalculateQuoteError::Other(err) => {
                tracing::error!(?err, "CalculateQuoteErrorWrapper");
                crate::api::internal_error_reply()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::api::response_body,
        alloy::primitives::Address,
        anyhow::anyhow,
        app_data::AppDataHash,
        bigdecimal::BigDecimal,
        chrono::{TimeZone, Utc},
        model::{
            order::{BuyTokenDestination, SellTokenSource},
            quote::{
                OrderQuote,
                OrderQuoteResponse,
                OrderQuoteSide,
                PriceQuality,
                QuoteSigningScheme,
                SellAmount,
                Validity,
            },
        },
        number::nonzero::NonZeroU256,
        reqwest::StatusCode,
        serde_json::json,
        shared::order_quoting::CalculateQuoteError,
        std::{str::FromStr, time::Duration},
    };

    #[test]
    fn deserializes_sell_after_fees_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountAfterFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
                "buyTokenBalance": "internal",
                "signingScheme": "presign",
                "priceQuality": "optimal"
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: Address::repeat_byte(0x01),
                sell_token: Address::repeat_byte(0x02),
                buy_token: Address::repeat_byte(0x03),
                receiver: None,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(1337).unwrap()
                    },
                },
                validity: Validity::To(0x12345678),
                app_data: AppDataHash([0x90; 32]).into(),
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Internal,
                signing_scheme: QuoteSigningScheme::PreSign {
                    onchain_order: false
                },
                price_quality: PriceQuality::Optimal,
                timeout: Default::default(),
            }
        );
    }

    #[test]
    fn deserializes_sell_before_fees_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountBeforeFee": "1337",
                "validFor": 1000,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
                "sellTokenBalance": "external",
                "priceQuality": "fast"
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: Address::repeat_byte(0x01),
                sell_token: Address::repeat_byte(0x02),
                buy_token: Address::repeat_byte(0x03),
                receiver: None,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::BeforeFee {
                        value: NonZeroU256::try_from(1337).unwrap()
                    },
                },
                validity: Validity::For(1000),
                app_data: AppDataHash([0x90; 32]).into(),
                sell_token_balance: SellTokenSource::External,
                price_quality: PriceQuality::Fast,
                ..Default::default()
            }
        );
    }

    #[test]
    fn deserializes_buy_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "receiver": "0x0404040404040404040404040404040404040404",
                "kind": "buy",
                "buyAmountAfterFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: Address::repeat_byte(0x01),
                sell_token: Address::repeat_byte(0x02),
                buy_token: Address::repeat_byte(0x03),
                receiver: Some(Address::repeat_byte(0x04)),
                side: OrderQuoteSide::Buy {
                    buy_amount_after_fee: NonZeroU256::try_from(1337).unwrap(),
                },
                validity: Validity::To(0x12345678),
                app_data: AppDataHash([0x90; 32]).into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn deserialize_minimum_parameters() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountAfterFee": "1337",
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: Address::repeat_byte(0x01),
                sell_token: Address::repeat_byte(0x02),
                buy_token: Address::repeat_byte(0x03),
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(1337).unwrap()
                    },
                },
                ..Default::default()
            }
        );
    }

    #[test]
    fn deserialize_small_timeout() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountAfterFee": "1337",
                "timeout": 1000,
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: Address::repeat_byte(0x01),
                sell_token: Address::repeat_byte(0x02),
                buy_token: Address::repeat_byte(0x03),
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(1337).unwrap()
                    },
                },
                timeout: Some(Duration::from_millis(1000)),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn post_quote_response_ok() {
        let quote = OrderQuote {
            sell_token: Default::default(),
            buy_token: Default::default(),
            receiver: None,
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            valid_to: 0,
            app_data: Default::default(),
            fee_amount: Default::default(),
            gas_amount: BigDecimal::from_str("100000").unwrap(),
            gas_price: BigDecimal::from_str("10000000000").unwrap(),
            sell_token_price: BigDecimal::from_str("0.0004").unwrap(),
            kind: Default::default(),
            partially_fillable: false,
            sell_token_balance: Default::default(),
            buy_token_balance: Default::default(),
            signing_scheme: Default::default(),
        };
        let order_quote_response = OrderQuoteResponse {
            quote,
            from: Address::ZERO,
            expiration: Utc.timestamp_millis_opt(0).unwrap(),
            id: Some(0),
            verified: false,
            protocol_fee_bps: Some("2".to_string()),
        };
        let response = (StatusCode::OK, Json(order_quote_response.clone())).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected = serde_json::to_value(order_quote_response).unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn post_quote_response_err() {
        let response =
            OrderQuoteError::CalculateQuote(CalculateQuoteError::Other(anyhow!("Uh oh - error")))
                .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_error = json!({"errorType": "InternalServerError", "description": ""});
        assert_eq!(body, expected_error);
        // There are many other FeeAndQuoteErrors, but writing a test for each
        // would follow the same pattern as this.
    }

    // Unique per-test labels keep the shared global metrics registry from
    // leaking counts between tests.
    #[test]
    fn guard_records_cancelled_when_dropped_without_finish() {
        let metrics = QuoteMetrics::instance(observe::metrics::get_storage_registry()).unwrap();
        let started = metrics
            .requests_started
            .with_label_values(&["dropped"])
            .get();
        let cancelled = metrics
            .requests_finished
            .with_label_values(&["dropped", "cancelled"])
            .get();

        // Dropping the guard without `finish` models a client-cancelled request.
        drop(QuoteRequestGuard::new("dropped"));

        assert_eq!(
            metrics
                .requests_started
                .with_label_values(&["dropped"])
                .get(),
            started + 1
        );
        assert_eq!(
            metrics
                .requests_finished
                .with_label_values(&["dropped", "cancelled"])
                .get(),
            cancelled + 1
        );
    }

    #[test]
    fn guard_records_outcome_after_finish() {
        let metrics = QuoteMetrics::instance(observe::metrics::get_storage_registry()).unwrap();
        let started = metrics
            .requests_started
            .with_label_values(&["finished"])
            .get();
        let success = metrics
            .requests_finished
            .with_label_values(&["finished", "success"])
            .get();
        let cancelled = metrics
            .requests_finished
            .with_label_values(&["finished", "cancelled"])
            .get();

        let mut guard = QuoteRequestGuard::new("finished");
        guard.finish("success");
        drop(guard);

        // started == sum(finished): exactly one started and one success, no
        // cancellation recorded once `finish` ran.
        assert_eq!(
            metrics
                .requests_started
                .with_label_values(&["finished"])
                .get(),
            started + 1
        );
        assert_eq!(
            metrics
                .requests_finished
                .with_label_values(&["finished", "success"])
                .get(),
            success + 1
        );
        assert_eq!(
            metrics
                .requests_finished
                .with_label_values(&["finished", "cancelled"])
                .get(),
            cancelled
        );
    }
}
