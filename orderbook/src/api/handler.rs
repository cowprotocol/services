use crate::storage::{AddOrderResult, Storage};
use anyhow::Result;
use chrono::prelude::{DateTime, FixedOffset, Utc};
use model::{
    order::{OrderCreation, OrderUid},
    u256_decimal,
};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use warp::{
    http::StatusCode,
    reply::{json, with_status, Json},
    Reply,
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

pub async fn add_order(
    storage: Arc<dyn Storage>,
    order: OrderCreation,
) -> Result<impl Reply, Infallible> {
    let (body, status_code) = match storage.add_order(order).await {
        Ok(AddOrderResult::Added(uid)) => (warp::reply::json(&uid), StatusCode::CREATED),
        Ok(AddOrderResult::DuplicatedOrder) => (
            error("DuplicatedOrder", "order already exists"),
            StatusCode::BAD_REQUEST,
        ),
        Ok(AddOrderResult::InvalidSignature) => (
            error("InvalidSignature", "invalid signature"),
            StatusCode::BAD_REQUEST,
        ),
        Ok(AddOrderResult::Forbidden) => (
            error("Forbidden", "Forbidden, your account is deny-listed"),
            StatusCode::FORBIDDEN,
        ),
        Ok(AddOrderResult::PastValidTo) => (
            error("PastValidTo", "validTo is in the past"),
            StatusCode::BAD_REQUEST,
        ),
        Ok(AddOrderResult::MissingOrderData) => (
            error(
                "MissingOrderData",
                "at least 1 field of orderCreation is missing, please check the field",
            ),
            StatusCode::BAD_REQUEST,
        ),
        Ok(AddOrderResult::InsufficientFunds) => (
            error(
                "InsufficientFunds",
                "order owner must have funds worth at least x in his account",
            ),
            StatusCode::BAD_REQUEST,
        ),
        Err(err) => {
            tracing::error!(?err, ?order, "add_order error");
            (internal_error(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    };
    Ok(with_status(body, status_code))
}

pub async fn get_orders(storage: Arc<dyn Storage>) -> Result<impl Reply, Infallible> {
    Ok(match storage.get_orders().await {
        Ok(orders) => with_status(json(&orders), StatusCode::OK),
        Err(err) => {
            tracing::error!(?err, "get_orders error");
            with_status(internal_error(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    })
}

pub async fn get_order_by_uid(
    uid: OrderUid,
    storage: Arc<dyn Storage>,
) -> Result<impl Reply, Infallible> {
    Ok(match storage.get_order(&uid).await {
        Ok(Some(order)) => with_status(json(&order), StatusCode::OK),
        Ok(None) => with_status(
            error("NotFound", "Order was not found"),
            StatusCode::NOT_FOUND,
        ),
        Err(err) => {
            tracing::error!(?err, ?uid, "get_order error");
            with_status(internal_error(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    })
}

#[allow(unused_variables)]
pub async fn get_fee_info(sell_token: H160) -> Result<impl Reply, Infallible> {
    let fee_info = FeeInfo {
        expiration_date: chrono::offset::Utc::now()
            + FixedOffset::east(STANDARD_VALIDITY_FOR_FEE_IN_SEC),
        minimal_fee: U256::zero(),
        fee_ratio: 0u32,
    };
    Ok(with_status(warp::reply::json(&fee_info), StatusCode::OK))
}
