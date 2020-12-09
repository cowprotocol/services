use crate::orderbook::{AddOrderError, OrderBook};

use chrono::prelude::{DateTime, FixedOffset, Utc};
use model::{u256_decimal, OrderCreation, OrderUid};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use warp::{
    http::StatusCode,
    reply::{json, with_status},
};

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

#[derive(PartialEq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderPostError {
    error_type: String,
    description: String,
}

#[derive(PartialEq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub struct UidResponse {
    uid: OrderUid,
}

pub async fn add_order(
    orderbook: Arc<OrderBook>,
    order: OrderCreation,
) -> Result<impl warp::Reply, Infallible> {
    let (body, status_code) = match orderbook.add_order(order).await {
        Ok(uid) => (warp::reply::json(&UidResponse { uid }), StatusCode::CREATED),
        Err(err) => {
            let (error_type, description, status_code) = match err {
                AddOrderError::DuplicatedOrder => (
                    "DuplicatedOrder",
                    "order already exists",
                    StatusCode::BAD_REQUEST,
                ),
                AddOrderError::InvalidSignature => (
                    "InvalidSignature",
                    "invalid signature",
                    StatusCode::BAD_REQUEST,
                ),
                AddOrderError::Forbidden => (
                    "Forbidden",
                    "Forbidden, your account is deny-listed",
                    StatusCode::FORBIDDEN,
                ),
                AddOrderError::PastValidTo => (
                    "PastValidTo",
                    "validTo is in the past",
                    StatusCode::BAD_REQUEST,
                ),
                AddOrderError::MissingOrderData => (
                    "MissingOrderData",
                    "at least 1 field of orderCreation is missing",
                    StatusCode::BAD_REQUEST,
                ),
                AddOrderError::InsufficientFunds => (
                    "InsufficientFunds",
                    "order owner must have funds worth at least x in his account",
                    StatusCode::BAD_REQUEST,
                ),
            };
            let error = OrderPostError {
                error_type: error_type.to_string(),
                description: description.to_string(),
            };
            (json(&error), status_code)
        }
    };
    Ok(with_status(body, status_code))
}

pub async fn get_orders(orderbook: Arc<OrderBook>) -> Result<impl warp::Reply, Infallible> {
    let orders = orderbook.get_orders().await;
    Ok(with_status(json(&orders), StatusCode::OK))
}

#[allow(unused_variables)]
pub async fn get_fee_info(sell_token: H160) -> Result<impl warp::Reply, Infallible> {
    let fee_info = FeeInfo {
        expiration_date: chrono::offset::Utc::now()
            + FixedOffset::east(STANDARD_VALIDITY_FOR_FEE_IN_SEC),
        minimal_fee: U256::zero(),
        fee_ratio: 0 as u32,
    };
    Ok(with_status(warp::reply::json(&fee_info), StatusCode::OK))
}
