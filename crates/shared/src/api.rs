use crate::metrics::get_metric_storage_registry;
use crate::price_estimation::PriceEstimationError;
use anyhow::{Error as anyhowError, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{convert::Infallible, fmt::Debug};
use warp::{
    hyper::StatusCode,
    reply::{json, with_status, Json, WithStatus},
    Filter, Rejection, Reply,
};

pub type ApiReply = WithStatus<Json>;

// We turn Rejection into Reply to workaround warp not setting CORS headers on rejections.
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let response = err.default_response();

    ApiMetrics::pub_instance()
        .requests_rejected
        .with_label_values(&[response.status().as_str()])
        .inc();

    Ok(response)
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "api")]
pub struct ApiMetrics {
    /// Number of completed API requests.
    #[metric(labels("method", "status_code"))]
    pub requests_complete: prometheus::IntCounterVec,

    /// Number of rejected API requests.
    #[metric(labels("status_code"))]
    pub requests_rejected: prometheus::IntCounterVec,

    /// Execution time for each API request.
    #[metric(labels("method"))]
    pub requests_duration_seconds: prometheus::HistogramVec,
}

impl ApiMetrics {
    /// We need this helper function to make `ApiMetrics` usable in crates using `shared`.
    pub fn pub_instance() -> &'static ApiMetrics {
        ApiMetrics::instance(get_metric_storage_registry()).unwrap()
    }
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
    match result {
        Ok(response) => with_status(warp::reply::json(&response), StatusCode::OK),
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
}
