use super::handler::{add_order, get_orders};
use crate::models::{Order, OrderBook};
use warp::Filter;

const MAX_JSON_BODY_PAYLOAD: u64 = 1024 * 16;

fn json_body() -> impl Filter<Extract = (Order,), Error = warp::Rejection> + Clone {
    // (rejecting huge payloads)...
    warp::body::content_length_limit(MAX_JSON_BODY_PAYLOAD).and(warp::body::json())
}

fn with_orderbook(
    orderbook: OrderBook,
) -> impl Filter<Extract = (OrderBook,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || orderbook.clone())
}

pub fn post_order(
    orderbook: OrderBook,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v1" / "orders")
        .and(warp::post())
        .and(json_body())
        .and(with_orderbook(orderbook))
        .and_then(add_order)
}

pub fn get(
    orderbook: OrderBook,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v1" / "orders")
        .and(warp::get())
        .and(with_orderbook(orderbook))
        .and_then(get_orders)
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use crate::models::{Order, SerializableOrderBook};
    use ethcontract::web3::types::U256;
    use warp::{http::StatusCode, test::request};

    #[tokio::test]
    async fn test_rending_of_get_request() {
        let orderbook = OrderBook::new();
        let order = Order::new_valid_test_order();
        let orderbook_api = orderbook.clone();
        orderbook.add_order(order.clone()).await;
        let filter = get(orderbook_api.clone());

        let result = request()
            .path("/v1/orders")
            .method("GET")
            .reply(&filter)
            .await;
        assert_eq!(result.status(), StatusCode::OK);
        let result_orderbook: SerializableOrderBook =
            serde_json::from_slice(result.body()).unwrap();
        let orderbook_orders = orderbook.orders.read().await;
        assert!(orderbook_orders.eq(&result_orderbook.orders));
    }
    #[tokio::test]
    async fn test_post_new_valid_order() {
        let orderbook = OrderBook::new();
        let filter = post_order(orderbook.clone());
        let order = Order::new_valid_test_order();
        let resp = request()
            .path("/v1/orders")
            .method("POST")
            .header("content-type", "application/json")
            .json(&order)
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), StatusCode::CREATED);
    }
    #[tokio::test]
    async fn test_post_new_invalid_order() {
        let orderbook = OrderBook::new();
        let filter = post_order(orderbook.clone());
        let mut order = Order::new_valid_test_order();
        order.sell_amount += U256::one();
        let resp = request()
            .path("/v1/orders")
            .method("POST")
            .header("content-type", "application/json")
            .json(&order)
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    #[tokio::test]
    async fn test_post_two_times_valid_order() {
        let orderbook = OrderBook::new();
        let filter = post_order(orderbook.clone());
        let order = Order::new_valid_test_order();
        warp::test::request()
            .path("/v1/orders")
            .method("POST")
            .header("content-type", "application/json")
            .json(&order)
            .reply(&filter)
            .await;
        let resp = request()
            .path("/v1/orders")
            .method("POST")
            .header("content-type", "application/json")
            .json(&order)
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
