use crate::{
    order_quoting::{CalculateQuoteError, OrderQuoteError},
    order_validation::PartialValidationError,
    price_estimation::PriceEstimationError,
};
use anyhow::{Error as anyhowError, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    convert::Infallible,
    fmt::Debug,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};
use warp::{
    filters::BoxedFilter,
    hyper::StatusCode,
    reply::{json, with_status, Json, WithStatus},
    Filter, Rejection, Reply,
};

pub type ApiReply = WithStatus<Json>;

// We turn Rejection into Reply to workaround warp not setting CORS headers on rejections.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let response = err.default_response();

    let metrics = ApiMetrics::instance(global_metrics::get_metric_storage_registry()).unwrap();
    metrics
        .requests_rejected
        .with_label_values(&[response.status().as_str()])
        .inc();

    Ok(response)
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "api")]
struct ApiMetrics {
    /// Number of completed API requests.
    #[metric(labels("method", "status_code"))]
    requests_complete: prometheus::IntCounterVec,

    /// Number of rejected API requests.
    #[metric(labels("status_code"))]
    requests_rejected: prometheus::IntCounterVec,

    /// Execution time for each API request.
    #[metric(labels("method"))]
    requests_duration_seconds: prometheus::HistogramVec,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Error<'a> {
    error_type: &'a str,
    description: &'a str,
    /// Additional arbitrary data that can be attached to an API error.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

pub fn error(error_type: &str, description: impl AsRef<str>) -> Json {
    json(&Error {
        error_type,
        description: description.as_ref(),
        data: None,
    })
}

pub fn rich_error(error_type: &str, description: impl AsRef<str>, data: impl Serialize) -> Json {
    let data = match serde_json::to_value(&data) {
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(?err, "failed to serialize error data");
            None
        }
    };

    json(&Error {
        error_type,
        description: description.as_ref(),
        data,
    })
}

pub fn internal_error(error: anyhowError) -> Json {
    tracing::error!(?error, "internal server error");
    json(&Error {
        error_type: "InternalServerError",
        description: "",
        data: None,
    })
}

pub fn convert_json_response<T, E>(result: Result<T, E>) -> WithStatus<Json>
where
    T: Serialize,
    E: IntoWarpReply + Debug,
{
    convert_json_response_with_status(result, StatusCode::OK)
}

pub fn convert_json_response_with_status<T, E>(
    result: Result<T, E>,
    status: StatusCode,
) -> WithStatus<Json>
where
    T: Serialize,
    E: IntoWarpReply + Debug,
{
    match result {
        Ok(response) => with_status(warp::reply::json(&response), status),
        Err(err) => err.into_warp_reply(),
    }
}

pub trait IntoWarpReply {
    fn into_warp_reply(self) -> ApiReply;
}

impl IntoWarpReply for anyhowError {
    fn into_warp_reply(self) -> ApiReply {
        with_status(internal_error(self), StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl IntoWarpReply for CalculateQuoteError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::Price(err) => err.into_warp_reply(),
            Self::SellAmountDoesNotCoverFee { fee_amount } => warp::reply::with_status(
                rich_error(
                    "SellAmountDoesNotCoverFee",
                    "The sell amount for the sell order is lower than the fee.",
                    serde_json::json!({ "fee_amount": fee_amount }),
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => err.into_warp_reply(),
        }
    }
}

impl IntoWarpReply for OrderQuoteError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::Validation(err) => err.into_warp_reply(),
            Self::CalculateQuote(err) => err.into_warp_reply(),
        }
    }
}

impl IntoWarpReply for PartialValidationError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::UnsupportedBuyTokenDestination(dest) => with_status(
                error("UnsupportedBuyTokenDestination", format!("Type {dest:?}")),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedSellTokenSource(src) => with_status(
                error("UnsupportedSellTokenSource", format!("Type {src:?}")),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedOrderType => with_status(
                error(
                    "UnsupportedOrderType",
                    "This order type is currently not supported",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::Forbidden => with_status(
                error("Forbidden", "Forbidden, your account is deny-listed"),
                StatusCode::FORBIDDEN,
            ),
            Self::InsufficientValidTo => with_status(
                error(
                    "InsufficientValidTo",
                    "validTo is not far enough in the future",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::ExcessiveValidTo => with_status(
                error("ExcessiveValidTo", "validTo is too far into the future"),
                StatusCode::BAD_REQUEST,
            ),
            Self::TransferEthToContract => with_status(
                error(
                    "TransferEthToContract",
                    "Sending Ether to smart contract wallets is currently not supported",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidNativeSellToken => with_status(
                error(
                    "InvalidNativeSellToken",
                    "The chain's native token (Ether/xDai) cannot be used as the sell token",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::SameBuyAndSellToken => with_status(
                error(
                    "SameBuyAndSellToken",
                    "Buy token is the same as the sell token.",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedSignature => with_status(
                error("UnsupportedSignature", "signing scheme is not supported"),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedToken(token) => with_status(
                error("UnsupportedToken", format!("Token address {token:?}")),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => with_status(
                internal_error(err.context("partial_validation")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

pub async fn response_body(response: warp::hyper::Response<warp::hyper::Body>) -> Vec<u8> {
    let mut body = response.into_body();
    let mut result = Vec::new();
    while let Some(bytes) = futures::StreamExt::next(&mut body).await {
        result.extend_from_slice(bytes.unwrap().as_ref());
    }
    result
}

const MAX_JSON_BODY_PAYLOAD: u64 = 1024 * 16;

pub fn extract_payload<T: DeserializeOwned + Send>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    // (rejecting huge payloads)...
    warp::body::content_length_limit(MAX_JSON_BODY_PAYLOAD).and(warp::body::json())
}

pub fn extract_payload_with_max_size<T: DeserializeOwned + Send>(
    max_size: u64,
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    // (rejecting huge payloads)...
    warp::body::content_length_limit(max_size).and(warp::body::json())
}

/// Sets up basic metrics, cors and proper log tracing for all routes.
pub fn finalize_router(
    routes: BoxedFilter<(ApiReply, &'static str)>,
    log_prefix: &'static str,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let metrics = ApiMetrics::instance(global_metrics::get_metric_storage_registry()).unwrap();
    let routes_with_metrics = warp::any()
        .map(Instant::now) // Start a timer at the beginning of response processing
        .and(routes) // Parse requests
        .map(|timer: Instant, reply: ApiReply, method: &'static str| {
            let response = reply.into_response();

            metrics
                .requests_complete
                .with_label_values(&[method, response.status().as_str()])
                .inc();
            metrics
                .requests_duration_seconds
                .with_label_values(&[method])
                .observe(timer.elapsed().as_secs_f64());

            response
        })
        .boxed();

    // Final setup

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS", "PUT", "PATCH"])
        .allow_headers(vec!["Origin", "Content-Type", "X-Auth-Token", "X-AppId"]);

    // Give each request a unique tracing span.
    // This allows us to match log statements across concurrent API requests. We
    // first try to read the request ID from our reverse proxy (this way we can
    // line up API request logs with Nginx requests) but fall back to an
    // internal counter.
    let internal_request_id = Arc::new(AtomicUsize::new(0));
    let tracing_span = warp::trace(move |info| {
        if let Some(header) = info.request_headers().get("X-Request-ID") {
            let request_id = String::from_utf8_lossy(header.as_bytes());
            tracing::info_span!("request", id = &*request_id)
        } else {
            let request_id = internal_request_id.fetch_add(1, Ordering::SeqCst);
            tracing::info_span!("request", id = request_id)
        }
    });

    routes_with_metrics
        .recover(handle_rejection)
        .with(cors)
        .with(warp::log::log(log_prefix))
        .with(tracing_span)
}

impl IntoWarpReply for PriceEstimationError {
    fn into_warp_reply(self) -> WithStatus<Json> {
        match self {
            Self::UnsupportedToken(token) => with_status(
                error("UnsupportedToken", format!("Token address {:?}", token)),
                StatusCode::BAD_REQUEST,
            ),
            Self::NoLiquidity => with_status(
                error("NoLiquidity", "not enough liquidity"),
                StatusCode::NOT_FOUND,
            ),
            Self::ZeroAmount => with_status(
                error("ZeroAmount", "Please use non-zero amount field"),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedOrderType => with_status(
                internal_error(anyhow::anyhow!("UnsupportedOrderType").context("price_estimation")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::RateLimited(_) => with_status(
                internal_error(
                    anyhow::anyhow!("price estimators temporarily inactive")
                        .context("price_estimation"),
                ),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::Other(err) => with_status(
                internal_error(err.context("price_estimation")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use chrono::{DateTime, NaiveDateTime, Utc};
    use model::quote::{OrderQuote, OrderQuoteResponse};
    use primitive_types::H160;
    use serde::ser;
    use serde_json::json;

    #[test]
    fn rich_errors_skip_unset_data_field() {
        assert_eq!(
            serde_json::to_value(&Error {
                error_type: "foo",
                description: "bar",
                data: None,
            })
            .unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
            }),
        );
        assert_eq!(
            serde_json::to_value(&Error {
                error_type: "foo",
                description: "bar",
                data: Some(json!(42)),
            })
            .unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
                "data": 42,
            }),
        );
    }

    #[tokio::test]
    async fn rich_errors_handle_serialization_errors() {
        struct AlwaysErrors;
        impl Serialize for AlwaysErrors {
            fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(ser::Error::custom("error"))
            }
        }

        let body = warp::hyper::body::to_bytes(
            rich_error("foo", "bar", AlwaysErrors)
                .into_response()
                .into_body(),
        )
        .await
        .unwrap();

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&*body).unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
            })
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
            kind: Default::default(),
            partially_fillable: false,
            sell_token_balance: Default::default(),
            buy_token_balance: Default::default(),
        };
        let order_quote_response = OrderQuoteResponse {
            quote,
            from: H160::zero(),
            expiration: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            id: Some(0),
        };
        let response = convert_json_response::<OrderQuoteResponse, OrderQuoteError>(Ok(
            order_quote_response.clone(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected = serde_json::to_value(order_quote_response).unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn post_quote_response_err() {
        let response = convert_json_response::<OrderQuoteResponse, OrderQuoteError>(Err(
            OrderQuoteError::CalculateQuote(CalculateQuoteError::Other(anyhow!("Uh oh - error"))),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_error = json!({"errorType": "InternalServerError", "description": ""});
        assert_eq!(body, expected_error);
        // There are many other FeeAndQuoteErrors, but writing a test for each would follow the same pattern as this.
    }
}
