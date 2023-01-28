use crate::price_estimation::PriceEstimationError;
use anyhow::Result;
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

impl ApiMetrics {
    // Status codes we care about in our application. Populated with:
    // `rg -oIN 'StatusCode::[A-Z_]+' | sort | uniq`.
    const INITIAL_STATUSES: &[StatusCode] = &[
        StatusCode::OK,
        StatusCode::CREATED,
        StatusCode::BAD_REQUEST,
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::NOT_FOUND,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::SERVICE_UNAVAILABLE,
    ];

    fn reset_requests_rejected(&self) {
        for status in Self::INITIAL_STATUSES {
            self.requests_rejected
                .with_label_values(&[status.as_str()])
                .reset();
        }
    }

    fn reset_requests_complete(&self, method: &str) {
        for status in Self::INITIAL_STATUSES {
            self.requests_complete
                .with_label_values(&[method, status.as_str()])
                .reset();
        }
    }

    fn on_request_completed(&self, method: &str, status: StatusCode, timer: Instant) {
        self.requests_complete
            .with_label_values(&[method, status.as_str()])
            .inc();
        self.requests_duration_seconds
            .with_label_values(&[method])
            .observe(timer.elapsed().as_secs_f64());
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

pub fn internal_error_reply() -> ApiReply {
    with_status(
        error("InternalServerError", ""),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
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
///
/// # Panics
///
/// This method panics if `routes` is empty.
pub fn finalize_router(
    routes: Vec<(&'static str, BoxedFilter<(ApiReply,)>)>,
    log_prefix: &'static str,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let metrics = ApiMetrics::instance(global_metrics::get_metric_storage_registry()).unwrap();
    metrics.reset_requests_rejected();
    for (method, _) in &routes {
        metrics.reset_requests_complete(method);
    }

    let router = routes
        .into_iter()
        .fold(
            Option::<BoxedFilter<(&'static str, ApiReply)>>::None,
            |router, (method, route)| {
                let route = route.map(move |result| (method, result)).untuple_one();
                let next = match router {
                    Some(router) => router.or(route).unify().boxed(),
                    None => route.boxed(),
                };
                Some(next)
            },
        )
        .expect("routes cannot be empty");

    let instrumented =
        warp::any()
            .map(Instant::now)
            .and(router)
            .map(|timer, method, reply: ApiReply| {
                let response = reply.into_response();
                metrics.on_request_completed(method, response.status(), timer);
                response
            });

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

    warp::path!("api" / ..)
        .and(instrumented)
        .recover(handle_rejection)
        .with(cors)
        .with(warp::log::log(log_prefix))
        .with(tracing_span)
}

impl IntoWarpReply for PriceEstimationError {
    fn into_warp_reply(self) -> WithStatus<Json> {
        match self {
            Self::UnsupportedToken(token) => with_status(
                error("UnsupportedToken", format!("Token address {token:?}")),
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
            Self::UnsupportedOrderType => {
                tracing::error!("PriceEstimaton::UnsupportedOrderType");
                internal_error_reply()
            }
            Self::RateLimited(_) => internal_error_reply(),
            Self::Other(err) => {
                tracing::error!(?err, "PriceEstimationError::Other");
                internal_error_reply()
            }
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
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
            })
        );
    }
}
