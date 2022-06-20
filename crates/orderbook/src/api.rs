mod cancel_order;
mod create_order;
mod get_auction;
mod get_fee_and_quote;
mod get_fee_info;
mod get_markets;
mod get_order_by_uid;
mod get_orders;
mod get_orders_by_tx;
mod get_solvable_orders;
mod get_solvable_orders_v2;
mod get_solver_competition;
mod get_trades;
mod get_user_orders;
mod post_quote;
mod post_solver_competition;
mod replace_order;

use crate::solver_competition::SolverCompetition;
use crate::{database::trades::TradeRetrieving, order_quoting::OrderQuoter, orderbook::Orderbook};
use anyhow::{Error as anyhowError, Result};
use serde::{de::DeserializeOwned, Serialize};
use shared::{metrics::get_metric_storage_registry, price_estimation::PriceEstimationError};
use std::{
    convert::Infallible,
    fmt::Debug,
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::Instant,
};
use warp::{
    hyper::StatusCode,
    reply::{json, with_status, Json, WithStatus},
    Filter, Rejection, Reply,
};

pub fn handle_all_routes(
    database: Arc<dyn TradeRetrieving>,
    orderbook: Arc<Orderbook>,
    quoter: Arc<OrderQuoter>,
    solver_competition: Arc<SolverCompetition>,
    solver_competition_auth: Option<String>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // Routes for api v1.

    // Note that we add a string with endpoint's name to all responses.
    // This string will be used later to report metrics.
    // It is not used to form the actual server response.

    let create_order = create_order::create_order(orderbook.clone())
        .map(|result| (result, "v1/create_order"))
        .boxed();
    let get_orders = get_orders::get_orders(orderbook.clone())
        .map(|result| (result, "v1/get_orders"))
        .boxed();
    let fee_info = get_fee_info::get_fee_info(quoter.fee_calculator.clone())
        .map(|result| (result, "v1/fee_info"))
        .boxed();
    let get_order = get_order_by_uid::get_order_by_uid(orderbook.clone())
        .map(|result| (result, "v1/get_order"))
        .boxed();
    let get_solvable_orders = get_solvable_orders::get_solvable_orders(orderbook.clone())
        .map(|result| (result, "v1/get_solvable_orders"))
        .boxed();
    let get_trades = get_trades::get_trades(database)
        .map(|result| (result, "v1/get_trades"))
        .boxed();
    let cancel_order = cancel_order::cancel_order(orderbook.clone())
        .map(|result| (result, "v1/cancel_order"))
        .boxed();
    let replace_order = replace_order::filter(orderbook.clone())
        .map(|result| (result, "v1/replace_order"))
        .boxed();
    let get_amount_estimate = get_markets::get_amount_estimate(quoter.price_estimator.clone())
        .map(|result| (result, "v1/get_amount_estimate"))
        .boxed();
    let get_fee_and_quote_sell = get_fee_and_quote::get_fee_and_quote_sell(quoter.clone())
        .map(|result| (result, "v1/get_fee_and_quote_sell"))
        .boxed();
    let get_fee_and_quote_buy = get_fee_and_quote::get_fee_and_quote_buy(quoter.clone())
        .map(|result| (result, "v1/get_fee_and_quote_buy"))
        .boxed();
    let get_user_orders = get_user_orders::get_user_orders(orderbook.clone())
        .map(|result| (result, "v1/get_user_orders"))
        .boxed();
    let get_orders_by_tx = get_orders_by_tx::get_orders_by_tx(orderbook.clone())
        .map(|result| (result, "v1/get_orders_by_tx"))
        .boxed();
    let post_quote = post_quote::post_quote(quoter)
        .map(|result| (result, "v1/post_quote"))
        .boxed();
    let get_auction = get_auction::get_auction(orderbook.clone())
        .map(|result| (result, "v1/auction"))
        .boxed();
    let get_solver_competition = get_solver_competition::get(solver_competition.clone())
        .map(|result| (result, "v1/solver_competition"))
        .boxed();
    let post_solver_competition =
        post_solver_competition::post(solver_competition, solver_competition_auth)
            .map(|result| (result, "v1/solver_competition"))
            .boxed();

    let routes_v1 = warp::path!("api" / "v1" / ..)
        .and(
            create_order
                .or(get_orders)
                .unify()
                .or(fee_info)
                .unify()
                .or(get_order)
                .unify()
                .or(get_solvable_orders)
                .unify()
                .or(get_trades)
                .unify()
                .or(cancel_order)
                .unify()
                .or(replace_order)
                .unify()
                .or(get_amount_estimate)
                .unify()
                .or(get_fee_and_quote_sell)
                .unify()
                .or(get_fee_and_quote_buy)
                .unify()
                .or(get_user_orders)
                .unify()
                .or(get_orders_by_tx)
                .unify()
                .or(post_quote)
                .unify()
                .or(get_auction)
                .unify()
                .or(get_solver_competition)
                .unify()
                .or(post_solver_competition)
                .unify(),
        )
        .untuple_one()
        .boxed();

    // Routes for api v2.

    let get_solvable_orders_v2 = get_solvable_orders_v2::get_solvable_orders(orderbook)
        .map(|result| (result, "v2/get_solvable_orders"))
        .boxed();

    let routes_v2 = warp::path!("api" / "v2" / ..)
        .and(get_solvable_orders_v2)
        .untuple_one();

    // Routes combined

    let routes = routes_v1.or(routes_v2).unify().boxed();

    // Metrics

    let metrics = ApiMetrics::instance(get_metric_storage_registry()).unwrap();
    let routes_with_metrics = warp::any()
        .map(Instant::now) // Start a timer at the beginning of response processing
        .and(routes) // Parse requests
        .map(|timer: Instant, reply: ApiReply, method: &str| {
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
        .with(warp::log::log("orderbook::api::request_summary"))
        .with(tracing_span)
}

pub type ApiReply = warp::reply::WithStatus<warp::reply::Json>;

// We turn Rejection into Reply to workaround warp not setting CORS headers on rejections.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let response = err.default_response();

    let metrics = ApiMetrics::instance(get_metric_storage_registry()).unwrap();
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
    requests_complete: prometheus::CounterVec,

    /// Number of rejected API requests.
    #[metric(labels("status_code"))]
    requests_rejected: prometheus::CounterVec,

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

fn error(error_type: &str, description: impl AsRef<str>) -> Json {
    json(&Error {
        error_type,
        description: description.as_ref(),
        data: None,
    })
}

fn rich_error(error_type: &str, description: impl AsRef<str>, data: impl Serialize) -> Json {
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

fn internal_error(error: anyhowError) -> Json {
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
async fn response_body(response: warp::hyper::Response<warp::hyper::Body>) -> Vec<u8> {
    let mut body = response.into_body();
    let mut result = Vec::new();
    while let Some(bytes) = futures::StreamExt::next(&mut body).await {
        result.extend_from_slice(bytes.unwrap().as_ref());
    }
    result
}

const MAX_JSON_BODY_PAYLOAD: u64 = 1024 * 16;

fn extract_payload<T: DeserializeOwned + Send>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    // (rejecting huge payloads)...
    warp::body::content_length_limit(MAX_JSON_BODY_PAYLOAD).and(warp::body::json())
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
