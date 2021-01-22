mod create_order;
mod get_fee_info;
mod get_order_by_uid;
mod get_orders;

use crate::storage::Storage;
use hex::{FromHex, FromHexError};
use model::h160_hexadecimal;
use primitive_types::H160;
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};
use warp::{
    reply::{json, Json},
    Filter, Reply,
};

pub fn handle_all_routes(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl Reply,), Error = warp::Rejection> + Clone {
    let order_creation = create_order::create_order(orderbook.clone());
    let order_getter = get_orders::get_orders(orderbook.clone());
    let fee_info = get_fee_info::get_fee_info();
    let order_by_uid = get_order_by_uid::get_order_by_uid(orderbook);
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

/// Wraps H160 with FromStr and Deserialize that can handle a `0x` prefix.
#[derive(Deserialize)]
#[serde(transparent)]
struct H160Wrapper(#[serde(with = "h160_hexadecimal")] H160);
impl FromStr for H160Wrapper {
    type Err = FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        Ok(H160Wrapper(H160(FromHex::from_hex(s)?)))
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
