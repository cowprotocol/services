use crate::orderbook::{AddOrderError, OrderBook};

use chrono::prelude::{DateTime, FixedOffset, Utc};
use model::{h160_hexadecimal, u256_decimal, OrderCreation};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use warp::http::StatusCode;

const STANDARD_VALIDITY_FOR_FEE_IN_SEC: i32 = 3600;

/// Fee struct being returned on fee API requests
#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeInfo {
    pub expiration_date: DateTime<Utc>,
    #[serde(with = "u256_decimal")]
    pub minimal_fee: U256,
    pub fee_ratio: u32,
}

/// Fee struct being returned on fee API requests
#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeRequestBody {
    #[serde(with = "h160_hexadecimal")]
    sell_token: H160,
}

pub async fn add_order(
    orderbook: Arc<OrderBook>,
    order: OrderCreation,
) -> Result<impl warp::Reply, Infallible> {
    let (body, status_code) = match orderbook.add_order(order).await {
        Ok(()) => ("ok", StatusCode::CREATED),
        Err(AddOrderError::AlreadyExists) => ("already exists", StatusCode::BAD_REQUEST),
        Err(AddOrderError::InvalidSignature) => ("invalid signature", StatusCode::BAD_REQUEST),
        Err(AddOrderError::PastNonce) => ("nonce is in the past", StatusCode::BAD_REQUEST),
        Err(AddOrderError::PastValidTo) => ("validTo is in the past", StatusCode::BAD_REQUEST),
    };
    Ok(warp::reply::with_status(body, status_code))
}

pub async fn get_orders(orderbook: Arc<OrderBook>) -> Result<impl warp::Reply, Infallible> {
    let orders = orderbook.get_orders().await;
    Ok(warp::reply::json(&orders))
}

#[allow(unused_variables)]
pub async fn get_fee_info(sell_token: FeeRequestBody) -> Result<impl warp::Reply, Infallible> {
    let fee_info = FeeInfo {
        expiration_date: chrono::offset::Utc::now()
            + FixedOffset::east(STANDARD_VALIDITY_FOR_FEE_IN_SEC),
        minimal_fee: U256::zero(),
        fee_ratio: 0 as u32,
    };
    Ok(warp::reply::json(&fee_info))
}
