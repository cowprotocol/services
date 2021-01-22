use super::H160Wrapper;
use crate::{database::OrderFilter, storage::Storage};
use anyhow::Result;
use model::order::Order;
use serde::Deserialize;
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply, Filter, Rejection, Reply};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    owner: Option<H160Wrapper>,
    sell_token: Option<H160Wrapper>,
    buy_token: Option<H160Wrapper>,
}

pub fn get_orders_request() -> impl Filter<Extract = (OrderFilter,), Error = Rejection> + Clone {
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
            return Ok(reply::with_status(
                super::internal_error(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };
    Ok(reply::with_status(reply::json(&orders), StatusCode::OK))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use hex_literal::hex;
    use primitive_types::H160;
    use warp::test::{request, RequestBuilder};

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
}
