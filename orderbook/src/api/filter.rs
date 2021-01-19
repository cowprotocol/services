use super::handler;
use crate::storage::Storage;
use hex::{FromHex, FromHexError};
use model::{
    h160_hexadecimal,
    order::{OrderCreation, OrderUid},
};
use primitive_types::H160;
use serde::Deserialize;
use std::{str::FromStr, sync::Arc};
use warp::Filter;

const MAX_JSON_BODY_PAYLOAD: u64 = 1024 * 16;

fn with_orderbook(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (Arc<dyn Storage>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || orderbook.clone())
}

fn extract_user_order() -> impl Filter<Extract = (OrderCreation,), Error = warp::Rejection> + Clone
{
    // (rejecting huge payloads)...
    warp::body::content_length_limit(MAX_JSON_BODY_PAYLOAD).and(warp::body::json())
}

pub fn create_order(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("orders")
        .and(warp::post())
        .and(with_orderbook(orderbook))
        .and(extract_user_order())
        .and_then(handler::add_order)
}

pub fn get_orders(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
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
        .and(with_orderbook(orderbook))
        .and_then(move |query: Query, orderbook| {
            handler::get_orders(
                orderbook,
                to_h160(query.owner),
                to_h160(query.sell_token),
                to_h160(query.buy_token),
            )
        })
}

pub fn get_order_by_uid(
    orderbook: Arc<dyn Storage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("orders" / OrderUid)
        .and(warp::get())
        .and(with_orderbook(orderbook))
        .and_then(handler::get_order_by_uid)
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

pub fn get_fee_info() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("tokens" / H160Wrapper / "fee")
        .and(warp::get())
        .map(|token: H160Wrapper| token.0)
        .and_then(handler::get_fee_info)
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use crate::{
        database::OrderFilter,
        storage::{AddOrderResult, InMemoryOrderBook as OrderBook},
    };
    use hex_literal::hex;
    use model::order::Order;
    use model::{order::OrderBuilder, DomainSeparator};
    use primitive_types::U256;
    use secp256k1::SecretKey;
    use serde_json::json;
    use warp::{http::StatusCode, test::request};
    use web3::signing::SecretKeyRef;

    #[tokio::test]
    async fn get_all_orders() {
        let orderbook = Arc::new(OrderBook::default());
        let filter = get_orders(orderbook.clone());
        let order = OrderCreation::default();
        orderbook.add_order(order).await.unwrap();
        let response = request().path("/orders").method("GET").reply(&filter).await;
        assert_eq!(response.status(), StatusCode::OK);
        let response_orders: Vec<Order> = serde_json::from_slice(response.body()).unwrap();
        let orderbook_orders = orderbook.get_orders(&OrderFilter::default()).await.unwrap();
        assert_eq!(response_orders, orderbook_orders);
    }

    #[tokio::test]
    async fn get_filtered_orders() {
        let domain_separator = DomainSeparator([0u8; 32]);
        let owner_key = SecretKey::from_slice(&hex!(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ))
        .unwrap();
        let sell = H160::from_slice(&hex!("0000000000000000000000000000000000000002"));
        let buy = H160::from_slice(&hex!("0000000000000000000000000000000000000003"));
        let orderbook = Arc::new(OrderBook::default());
        let filter = get_orders(orderbook.clone());
        let order = OrderBuilder::default()
            .with_sell_token(sell)
            .with_buy_token(buy)
            .sign_with(&domain_separator, SecretKeyRef::from(&owner_key))
            .build();
        let owner = order.order_meta_data.owner;
        orderbook.add_order(order.order_creation).await.unwrap();

        let orders = move |path: String| {
            let filter = filter.clone();
            async move {
                let response = request()
                    .path(path.as_str())
                    .method("GET")
                    .reply(&filter)
                    .await;
                assert_eq!(response.status(), StatusCode::OK);
                let response_orders: Vec<Order> = serde_json::from_slice(response.body()).unwrap();
                response_orders.len()
            }
        };
        let zero = H160::zero();
        assert_eq!(orders("/orders".to_string()).await, 1);
        assert_eq!(orders(format!("/orders?owner=0x{:x}", zero)).await, 0);
        assert_eq!(orders(format!("/orders?sellToken=0x{:x}", zero)).await, 0);
        assert_eq!(orders(format!("/orders?buyToken=0x{:x}", zero)).await, 0);
        assert_eq!(orders(format!("/orders?owner=0x{:x}", owner)).await, 1);
        assert_eq!(orders(format!("/orders?sellToken=0x{:x}", sell)).await, 1);
        assert_eq!(orders(format!("/orders?buyToken=0x{:x}", buy)).await, 1);
        assert_eq!(
            orders(format!(
                "/orders?owner=0x{:x}&sellToken=0x{:x}&buyToken=0x{:x}",
                owner, sell, buy
            ))
            .await,
            1
        );
        assert_eq!(
            orders(format!(
                "/orders?owner=0x{:x}&sellToken=0x{:x}&buyToken=0x{:x}",
                owner, sell, zero
            ))
            .await,
            0
        );
    }

    #[tokio::test]
    async fn get_order_by_uid_() {
        let orderbook = Arc::new(OrderBook::default());
        let filter = get_order_by_uid(orderbook.clone());
        let order_creation = OrderCreation::default();
        let uid = match orderbook.add_order(order_creation).await.unwrap() {
            AddOrderResult::Added(uid) => uid,
            _ => panic!("unexpected result"),
        };
        let response = request()
            .path(&format!("/orders/{:}", uid))
            .method("GET")
            .reply(&filter)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let response_orders: Order = serde_json::from_slice(response.body()).unwrap();
        let orderbook_orders = orderbook.get_orders(&OrderFilter::default()).await.unwrap();
        assert_eq!(response_orders, orderbook_orders[0]);
    }
    #[tokio::test]
    async fn get_order_by_uid_for_non_existent_order_() {
        let orderbook = Arc::new(OrderBook::default());
        let filter = get_order_by_uid(orderbook.clone());
        let uid = OrderUid([0u8; 56]);
        let response = request()
            .path(&format!("/orders/{:}", uid))
            .method("GET")
            .reply(&filter)
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_fee_info_() {
        let filter = get_fee_info();
        let sell_token = String::from("0x000000000000000000000000000000000000000a");
        let path_string = format!("/tokens/{}/fee", sell_token);
        let post = || async {
            request()
                .path(&path_string)
                .method("GET")
                .reply(&filter)
                .await
        };
        let response = post().await;
        let body: handler::FeeInfo = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
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
