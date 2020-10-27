use crate::orderbook::{AddOrderError, OrderBook};
use model::UserOrder;
use std::{convert::Infallible, sync::Arc};
use warp::http::StatusCode;

pub async fn add_order(
    orderbook: Arc<OrderBook>,
    order: UserOrder,
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
