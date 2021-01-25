use super::handler;
use crate::{database::OrderFilter, storage::Storage};
use anyhow::Result;
use chrono::{DateTime, FixedOffset, Utc};
use hex::{FromHex, FromHexError};
use model::{
    h160_hexadecimal,
    order::{Order, OrderCreation, OrderUid},
    u256_decimal,
};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, str::FromStr, sync::Arc};
use warp::{
    http::StatusCode,
    reply::{json, with_status},
    Filter, Rejection, Reply,
};

const MAX_JSON_BODY_PAYLOAD: u64 = 1024 * 16;

fn with_orderbook(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (Arc<dyn Storage>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || orderbook.clone())
}

fn extract_user_order() -> impl Filter<Extract = (OrderCreation,), Error = Rejection> + Clone {
    // (rejecting huge payloads)...
    warp::body::content_length_limit(MAX_JSON_BODY_PAYLOAD).and(warp::body::json())
}

pub fn create_order(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path!("orders")
        .and(warp::post())
        .and(with_orderbook(orderbook))
        .and(extract_user_order())
        .and_then(handler::add_order)
}

pub fn get_orders_request() -> impl Filter<Extract = (OrderFilter,), Error = Rejection> + Clone {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Query {
        owner: Option<H160Wrapper>,
        sell_token: Option<H160Wrapper>,
        buy_token: Option<H160Wrapper>,
    }

    let to_h160 = |option: Option<H160Wrapper>| option.map(|wrapper| wrapper.0);

    warp::path!("orders")
        .and(warp::get())
        .and(warp::query::<Query>())
        .map(move |query: Query| OrderFilter {
            owner: to_h160(query.owner),
            sell_token: to_h160(query.sell_token),
            buy_token: to_h160(query.buy_token),
            exclude_fully_executed: true,
            exclude_invalidated: true,
            ..Default::default()
        })
}

pub fn get_orders_response(result: Result<Vec<Order>>) -> impl Reply {
    let orders = match result {
        Ok(orders) => orders,
        Err(err) => {
            tracing::error!(?err, "get_orders error");
            return Ok(with_status(
                super::internal_error(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };
    Ok(with_status(json(&orders), StatusCode::OK))
}

pub fn get_orders(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_orders_request().and_then(move |order_filter| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_orders(&order_filter).await;
            Result::<_, Infallible>::Ok(get_orders_response(result))
        }
    })
}

pub fn get_order_by_uid_request() -> impl Filter<Extract = (OrderFilter,), Error = Rejection> + Clone
{
    warp::path!("orders" / OrderUid)
        .and(warp::get())
        .map(|uid| OrderFilter {
            uid: Some(uid),
            ..Default::default()
        })
}

pub fn get_order_by_uid_response(result: Result<Vec<Order>>) -> impl Reply {
    let orders = match result {
        Ok(orders) => orders,
        Err(err) => {
            tracing::error!(?err, "get_orders error");
            return Ok(with_status(
                super::internal_error(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };
    Ok(match orders.first() {
        Some(order) => with_status(json(&order), StatusCode::OK),
        None => with_status(
            super::error("NotFound", "Order was not found"),
            StatusCode::NOT_FOUND,
        ),
    })
}

pub fn get_order_by_uid(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_order_by_uid_request().and_then(move |order_filter| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_orders(&order_filter).await;
            Result::<_, Infallible>::Ok(get_order_by_uid_response(result))
        }
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

pub fn get_fee_info_request() -> impl Filter<Extract = (H160,), Error = Rejection> + Clone {
    warp::path!("tokens" / H160Wrapper / "fee")
        .and(warp::get())
        .map(|token: H160Wrapper| token.0)
}

/// Fee struct being returned on fee API requests
#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeInfo {
    pub expiration_date: DateTime<Utc>,
    #[serde(with = "u256_decimal")]
    pub minimal_fee: U256,
    pub fee_ratio: u32,
}

pub fn get_fee_info_response() -> impl Reply {
    const STANDARD_VALIDITY_FOR_FEE_IN_SEC: i32 = 3600;
    let fee_info = FeeInfo {
        expiration_date: chrono::offset::Utc::now()
            + FixedOffset::east(STANDARD_VALIDITY_FOR_FEE_IN_SEC),
        minimal_fee: U256::zero(),
        fee_ratio: 0u32,
    };
    with_status(warp::reply::json(&fee_info), StatusCode::OK)
}

pub fn get_fee_info() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_fee_info_request()
        .and_then(|_token| async move { Result::<_, Infallible>::Ok(get_fee_info_response()) })
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use crate::storage::InMemoryOrderBook as OrderBook;
    use futures::StreamExt;
    use hex_literal::hex;
    use model::order::Order;
    use primitive_types::U256;
    use serde_json::json;
    use warp::{
        http::StatusCode,
        hyper::{Body, Response},
        test::{request, RequestBuilder},
    };

    async fn response_body(response: Response<Body>) -> Vec<u8> {
        let mut body = response.into_body();
        let mut result = Vec::new();
        while let Some(bytes) = body.next().await {
            result.extend_from_slice(bytes.unwrap().as_ref());
        }
        result
    }

    #[tokio::test]
    async fn get_orders_request_ok() {
        let order_filter = |request: RequestBuilder| async move {
            let filter = get_orders_request();
            request.method("GET").filter(&filter).await
        };

        let result = order_filter(request().path("/orders")).await.unwrap();
        assert_eq!(result.owner, None);
        assert_eq!(result.buy_token, None);
        assert_eq!(result.sell_token, None);

        let owner = H160::from_slice(&hex!("0000000000000000000000000000000000000001"));
        let sell = H160::from_slice(&hex!("0000000000000000000000000000000000000002"));
        let buy = H160::from_slice(&hex!("0000000000000000000000000000000000000003"));
        let path = format!(
            "/orders?owner=0x{:x}&sellToken=0x{:x}&buyToken=0x{:x}",
            owner, sell, buy
        );
        let request = request().path(path.as_str());
        let result = order_filter(request).await.unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.buy_token, Some(buy));
        assert_eq!(result.sell_token, Some(sell));
    }

    #[tokio::test]
    async fn get_orders_response_ok() {
        let orders = vec![Order::default()];
        let response = get_orders_response(Ok(orders.clone())).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_orders: Vec<Order> = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response_orders, orders);
    }

    #[tokio::test]
    async fn get_order_by_uid_request_ok() {
        let uid = OrderUid::default();
        let request = request().path(&format!("/orders/{:}", uid)).method("GET");
        let filter = get_order_by_uid_request();
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result.uid, Some(uid));
    }

    #[tokio::test]
    async fn get_order_by_uid_response_ok() {
        let orders = vec![Order::default()];
        let response = get_order_by_uid_response(Ok(orders.clone())).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_order: Order = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response_order, orders[0]);
    }

    #[tokio::test]
    async fn get_order_by_uid_response_non_existent() {
        let response = get_order_by_uid_response(Ok(Vec::new())).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_fee_info_request_ok() {
        let filter = get_fee_info_request();
        let token = String::from("0x0000000000000000000000000000000000000001");
        let path_string = format!("/tokens/{}/fee", token);
        let request = request().path(&path_string).method("GET");
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result, H160::from_low_u64_be(1));
    }

    #[tokio::test]
    async fn get_fee_info_response_() {
        let response = get_fee_info_response().into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let body: FeeInfo = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(body.minimal_fee, U256::zero());
        assert_eq!(body.fee_ratio, 0);
        assert!(body.expiration_date.gt(&chrono::offset::Utc::now()))
    }

    #[tokio::test]
    async fn create_order_route() {
        let orderbook = Arc::new(OrderBook::default());
        let filter = create_order(orderbook.clone());
        let order = OrderCreation::default();
        let expected_uid = json!(
            "0xbd185ee633752c56b3eabec61259e8a65c765943665a2c17ad8b74a119e5f1ca7e5f4552091a69125d5dfcb7b8c2659029395bdfffffffff"
        );
        let post = || async {
            request()
                .path("/orders")
                .method("POST")
                .header("content-type", "application/json")
                .json(&order)
                .reply(&filter)
                .await
        };
        let response = post().await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let body: serde_json::Value = serde_json::from_slice(response.body()).unwrap();

        assert_eq!(body, expected_uid);
        // Posting again should fail because order already exists.
        let response = post().await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body: serde_json::Value = serde_json::from_slice(response.body()).unwrap();
        let expected_error =
            json!({"errorType": "DuplicatedOrder", "description": "order already exists"});
        assert_eq!(body, expected_error);
    }
}
