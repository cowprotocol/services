use crate::models::{Order, OrderBook, SerializableOrderBook};
use anyhow::Result;
use std::convert::Infallible;
use warp::http;

pub async fn add_order(order: Order, orderbook: OrderBook) -> Result<impl warp::Reply, Infallible> {
    if !order.validate_order().unwrap_or(false) {
        Ok(warp::reply::with_status(
            "Order does not have a valid signature",
            http::StatusCode::BAD_REQUEST,
        ))
    } else {
        let add_order_success = orderbook.add_order(order.clone()).await;
        if add_order_success {
            Ok(warp::reply::with_status(
                "Added order to the orderbook",
                http::StatusCode::CREATED,
            ))
        } else {
            Ok(warp::reply::with_status(
                "Did not add order to the orderbook, as it was already in the orderbook",
                http::StatusCode::BAD_REQUEST,
            ))
        }
    }
}

pub async fn get_orders(orderbook: OrderBook) -> Result<impl warp::Reply, Infallible> {
    let orderbook_struct = SerializableOrderBook::new(orderbook.orders.read().await.clone());
    Ok(warp::reply::json(&orderbook_struct))
}
