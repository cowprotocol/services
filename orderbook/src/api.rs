mod filter;
mod handler;

use crate::storage::Storage;
use serde::Serialize;
use std::sync::Arc;
use warp::{
    reply::{json, Json},
    Filter, Reply,
};

pub fn handle_all_routes(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl Reply,), Error = warp::Rejection> + Clone {
    let order_creation = filter::create_order(orderbook.clone());
    let order_getter = filter::get_orders(orderbook.clone());
    let fee_info = filter::get_fee_info();
    let order_by_uid = filter::get_order_by_uid(orderbook);
    warp::path!("api" / "v1" / ..).and(
        order_creation
            .or(order_getter)
            .or(fee_info)
            .or(order_by_uid),
    )
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
