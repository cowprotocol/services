use super::handler;
use crate::orderbook::OrderBook;
use model::UserOrder;
use std::sync::Arc;
use warp::Filter;

const MAX_JSON_BODY_PAYLOAD: u64 = 1024 * 16;

fn with_orderbook(
    orderbook: Arc<OrderBook>,
) -> impl Filter<Extract = (Arc<OrderBook>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || orderbook.clone())
}

fn extract_user_order() -> impl Filter<Extract = (UserOrder,), Error = warp::Rejection> + Clone {
    // (rejecting huge payloads)...
    warp::body::content_length_limit(MAX_JSON_BODY_PAYLOAD).and(warp::body::json())
}

pub fn create_order(
    orderbook: Arc<OrderBook>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "orders")
        .and(warp::post())
        .and(with_orderbook(orderbook))
        .and(extract_user_order())
        .and_then(handler::add_order)
}

pub fn get_orders(
    orderbook: Arc<OrderBook>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "orders")
        .and(warp::get())
        .and(with_orderbook(orderbook))
        .and_then(handler::get_orders)
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use model::Order;
    use warp::{http::StatusCode, test::request};

    #[tokio::test]
    async fn get_orders_() {
        let orderbook = Arc::new(OrderBook::default());
        let filter = get_orders(orderbook.clone());
        let order = UserOrder::default();
        orderbook.add_order(order).await.unwrap();
        let response = request()
            .path("/api/v1/orders")
            .method("GET")
            .reply(&filter)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let response_orders: Vec<Order> = serde_json::from_slice(response.body()).unwrap();
        let orderbook_orders = orderbook.get_orders().await;
        assert_eq!(response_orders, orderbook_orders);
    }
    #[tokio::test]
    async fn create_order_() {
        let orderbook = Arc::new(OrderBook::default());
        let filter = create_order(orderbook.clone());
        let order = UserOrder::default();
        let post = || async {
            request()
                .path("/api/v1/orders")
                .method("POST")
                .header("content-type", "application/json")
                .json(&order)
                .reply(&filter)
                .await
        };
        let response = post().await;
        assert_eq!(response.status(), StatusCode::CREATED);
        // Posting again should fail because order already exists.
        let response = post().await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
