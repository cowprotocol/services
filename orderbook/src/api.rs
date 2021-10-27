mod cancel_order;
mod create_order;
mod get_fee_and_quote;
mod get_fee_info;
mod get_markets;
mod get_order_by_uid;
mod get_orders;
mod get_orders_by_tx;
mod get_solvable_orders;
mod get_solvable_orders_v2;
mod get_trades;
mod get_user_orders;
pub mod order_validation;
pub mod post_quote;

use crate::{
    api::post_quote::OrderQuoter, database::trades::TradeRetrieving, orderbook::Orderbook,
};
use anyhow::{Error as anyhowError, Result};
use serde::{de::DeserializeOwned, Serialize};
use shared::{metrics::get_metric_storage_registry, price_estimation::PriceEstimationError};
use std::fmt::Debug;
use std::{convert::Infallible, sync::Arc};
use warp::{
    hyper::StatusCode,
    reply::{json, with_status, Json, WithStatus},
    Filter, Rejection, Reply,
};

pub fn handle_all_routes(
    database: Arc<dyn TradeRetrieving>,
    orderbook: Arc<Orderbook>,
    quoter: Arc<OrderQuoter>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let create_order = create_order::create_order(orderbook.clone());
    let get_orders = get_orders::get_orders(orderbook.clone());
    let fee_info = get_fee_info::get_fee_info(quoter.fee_calculator.clone());
    let get_order = get_order_by_uid::get_order_by_uid(orderbook.clone());
    let get_solvable_orders = get_solvable_orders::get_solvable_orders(orderbook.clone());
    let get_solvable_orders_v2 = get_solvable_orders_v2::get_solvable_orders(orderbook.clone());
    let get_trades = get_trades::get_trades(database);
    let cancel_order = cancel_order::cancel_order(orderbook.clone());
    let get_amount_estimate = get_markets::get_amount_estimate(quoter.price_estimator.clone());
    let get_fee_and_quote_sell = get_fee_and_quote::get_fee_and_quote_sell(quoter.clone());
    let get_fee_and_quote_buy = get_fee_and_quote::get_fee_and_quote_buy(quoter.clone());
    let get_user_orders = get_user_orders::get_user_orders(orderbook.clone());
    let get_orders_by_tx = get_orders_by_tx::get_orders_by_tx(orderbook);
    let post_quote = post_quote::post_quote(quoter);
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS", "PUT", "PATCH"])
        .allow_headers(vec!["Origin", "Content-Type", "X-Auth-Token", "X-AppId"]);
    let routes_with_labels = (warp::path!("api" / "v1" / ..)
        .and(create_order.with(handle_metrics("create_order"))))
    .or(warp::path!("api" / "v1" / ..).and(get_orders.with(handle_metrics("get_orders"))))
    .or(warp::path!("api" / "v1" / ..).and(fee_info.with(handle_metrics("fee_info"))))
    .or(warp::path!("api" / "v1" / ..).and(get_order.with(handle_metrics("get_order"))))
    .or(warp::path!("api" / "v1" / ..)
        .and(get_solvable_orders.with(handle_metrics("get_solvable_orders"))))
    .or(warp::path!("api" / "v2" / ..)
        .and(get_solvable_orders_v2.with(handle_metrics("get_solvable_orders"))))
    .or(warp::path!("api" / "v1" / ..).and(get_trades.with(handle_metrics("get_trades"))))
    .or(warp::path!("api" / "v1" / ..).and(cancel_order.with(handle_metrics("cancel_order"))))
    .or(warp::path!("api" / "v1" / ..)
        .and(get_amount_estimate.with(handle_metrics("get_amount_estimate"))))
    .or(warp::path!("api" / "v1" / ..)
        .and(get_fee_and_quote_sell.with(handle_metrics("get_fee_and_quote_sell"))))
    .or(warp::path!("api" / "v1" / ..)
        .and(get_fee_and_quote_buy.with(handle_metrics("get_fee_and_quote_buy"))))
    .or(warp::path!("api" / "v1" / ..).and(get_user_orders.with(handle_metrics("get_user_orders"))))
    .or(warp::path!("api" / "v1" / ..).and(get_orders_by_tx.with(handle_metrics("get_user_by_tx"))))
    .or(warp::path!("api" / "v1" / ..).and(post_quote.with(handle_metrics("post_quote"))));

    routes_with_labels.recover(handle_rejection).with(cors)
}

// We turn Rejection into Reply to workaround warp not setting CORS headers on rejections.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    Ok(err.default_response())
}

fn handle_metrics(endpoint: impl Into<String>) -> warp::log::Log<impl Fn(warp::log::Info) + Clone> {
    let metrics = ApiMetrics::instance(get_metric_storage_registry(), endpoint.into()).unwrap();

    warp::log::custom(move |info: warp::log::Info| {
        metrics
            .requests_complete
            .with_label_values(&[info.status().as_str()])
            .inc();
        metrics
            .requests_duration_seconds
            .observe(info.elapsed().as_secs_f64());
    })
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "api", labels("endpoint"))]
struct ApiMetrics {
    /// Number of completed API requests.
    #[metric(labels("status_code"))]
    requests_complete: prometheus::CounterVec,

    /// Execution time for each API request.
    requests_duration_seconds: prometheus::Histogram,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Error<'a> {
    error_type: &'a str,
    description: &'a str,
}

fn error(error_type: &str, description: impl AsRef<str>) -> Json {
    json(&Error {
        error_type,
        description: description.as_ref(),
    })
}

fn internal_error(error: anyhowError) -> Json {
    tracing::error!(?error, "internal server error");
    json(&Error {
        error_type: "InternalServerError",
        description: "",
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
    fn into_warp_reply(self) -> WithStatus<Json>;
}

impl IntoWarpReply for anyhowError {
    fn into_warp_reply(self) -> WithStatus<Json> {
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
