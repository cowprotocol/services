mod cancel_order;
mod create_order;
mod get_fee_and_quote;
mod get_fee_info;
mod get_markets;
mod get_order_by_uid;
mod get_orders;
mod get_solvable_orders;
mod get_trades;

use crate::{
    database::trades::TradeRetrieving, fee::EthAwareMinFeeCalculator, orderbook::Orderbook,
};
use anyhow::Error as anyhowError;
use serde::de::DeserializeOwned;
use serde::Serialize;
use shared::price_estimate::PriceEstimating;
use std::{convert::Infallible, sync::Arc};
use warp::{
    hyper::StatusCode,
    reply::{json, with_status, Json, WithStatus},
    Filter, Rejection, Reply,
};

pub fn handle_all_routes(
    database: Arc<dyn TradeRetrieving>,
    orderbook: Arc<Orderbook>,
    fee_calculator: Arc<EthAwareMinFeeCalculator>,
    price_estimator: Arc<dyn PriceEstimating>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let create_order = create_order::create_order(orderbook.clone());
    let get_orders = get_orders::get_orders(orderbook.clone());
    let legacy_fee_info = get_fee_info::legacy_get_fee_info(fee_calculator.clone());
    let fee_info = get_fee_info::get_fee_info(fee_calculator.clone());
    let get_order = get_order_by_uid::get_order_by_uid(orderbook.clone());
    let get_solvable_orders = get_solvable_orders::get_solvable_orders(orderbook.clone());
    let get_trades = get_trades::get_trades(database);
    let cancel_order = cancel_order::cancel_order(orderbook);
    let get_amount_estimate = get_markets::get_amount_estimate(price_estimator.clone());
    let get_fee_and_quote_sell =
        get_fee_and_quote::get_fee_and_quote_sell(fee_calculator.clone(), price_estimator.clone());
    let get_fee_and_quote_buy =
        get_fee_and_quote::get_fee_and_quote_buy(fee_calculator, price_estimator.clone());
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS", "PUT", "PATCH"])
        .allow_headers(vec!["Origin", "Content-Type", "X-Auth-Token", "X-AppId"]);
    let routes_with_labels = warp::path!("api" / "v1" / ..).and(
        (create_order)
            .or(get_orders)
            .or(fee_info)
            .or(legacy_fee_info)
            .or(get_order)
            .or(get_solvable_orders)
            .or(get_trades)
            .or(cancel_order)
            .or(get_amount_estimate)
            .or(get_fee_and_quote_sell)
            .or(get_fee_and_quote_buy),
    );

    routes_with_labels
        .with(warp::log::custom(handle_metrics))
        .recover(handle_rejection)
        .with(cors)
}

// We turn Rejection into Reply to workaround warp not setting CORS headers on rejections.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    Ok(err.default_response())
}

fn handle_metrics(info: warp::log::Info) {
    let url = info.path();

    let metrics = ApiMetrics::instance();
    metrics
        .requests_complete
        .with_label_values(&[url, info.status().as_str()])
        .inc();
    metrics
        .requests_duration_seconds
        .with_label_values(&[url])
        .observe(info.elapsed().as_secs_f64());
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "api")]
struct ApiMetrics {
    /// Number of completed API requests.
    #[metric(labels("url", "status_code"))]
    requests_complete: prometheus::CounterVec,

    /// Execution time for each API request.
    #[metric(labels("url"))]
    requests_duration_seconds: prometheus::HistogramVec,
}

impl ApiMetrics {
    fn instance() -> &'static Self {
        lazy_static::lazy_static! {
            static ref INSTANCE: ApiMetrics =
                ApiMetrics::new(shared::metrics::get_metrics_registry()).unwrap();
        }

        &INSTANCE
    }
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

fn internal_error() -> Json {
    json(&Error {
        error_type: "InternalServerError",
        description: "",
    })
}

pub fn convert_get_orders_error_to_reply(err: anyhowError) -> WithStatus<Json> {
    tracing::error!(?err, "get_orders error");
    with_status(internal_error(), StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn convert_get_trades_error_to_reply(err: anyhowError) -> WithStatus<Json> {
    tracing::error!(?err, "get_trades error");
    with_status(internal_error(), StatusCode::INTERNAL_SERVER_ERROR)
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
