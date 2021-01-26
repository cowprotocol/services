use crate::database::OrderFilter;
use crate::orderbook::Orderbook;
use anyhow::Result;
use model::order::{Order, OrderUid};
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply, Filter, Rejection, Reply};

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
            return Ok(reply::with_status(
                super::internal_error(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };
    Ok(match orders.first() {
        Some(order) => reply::with_status(reply::json(&order), StatusCode::OK),
        None => reply::with_status(
            super::error("NotFound", "Order was not found"),
            StatusCode::NOT_FOUND,
        ),
    })
}

pub fn get_order_by_uid(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_order_by_uid_request().and_then(move |order_filter| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_orders(&order_filter).await;
            Result::<_, Infallible>::Ok(get_order_by_uid_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use warp::test::request;

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
}
