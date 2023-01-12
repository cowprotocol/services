use crate::orderbook::Orderbook;
use anyhow::Result;
use model::order::{Order, OrderUid};
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply, Filter, Rejection};

pub fn get_order_by_uid_request() -> impl Filter<Extract = (OrderUid,), Error = Rejection> + Clone {
    warp::path!("v1" / "orders" / OrderUid).and(warp::get())
}

pub fn get_order_by_uid_response(result: Result<Option<Order>>) -> super::ApiReply {
    let order = match result {
        Ok(order) => order,
        Err(err) => {
            tracing::error!(?err, "get_order_by_uid_response");
            return shared::api::internal_error_reply();
        }
    };
    match order {
        Some(order) => reply::with_status(reply::json(&order), StatusCode::OK),
        None => reply::with_status(
            super::error("NotFound", "Order was not found"),
            StatusCode::NOT_FOUND,
        ),
    }
}

pub fn get_order_by_uid(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_order_by_uid_request().and_then(move |uid| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_order(&uid).await;
            Result::<_, Infallible>::Ok(get_order_by_uid_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::api::response_body;
    use warp::{test::request, Reply};

    #[tokio::test]
    async fn get_order_by_uid_request_ok() {
        let uid = OrderUid::default();
        let request = request()
            .path(&format!("/v1/orders/{:}", uid))
            .method("GET");
        let filter = get_order_by_uid_request();
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result, uid);
    }

    #[tokio::test]
    async fn get_order_by_uid_response_ok() {
        let order = Order::default();
        let response = get_order_by_uid_response(Ok(Some(order.clone()))).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_order: Order = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response_order, order);
    }

    #[tokio::test]
    async fn get_order_by_uid_response_non_existent() {
        let response = get_order_by_uid_response(Ok(None)).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
