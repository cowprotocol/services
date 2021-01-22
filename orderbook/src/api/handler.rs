use super::{error, internal_error};
use crate::storage::{AddOrderResult, Storage};
use anyhow::Result;
use model::order::OrderCreation;
use std::{convert::Infallible, sync::Arc};
use warp::{http::StatusCode, reply::with_status, Reply};

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
