use crate::{
    api::{extract_payload, IntoWarpReply},
    orderbook::{AddOrderError, Orderbook},
};
use anyhow::Result;
use model::order::{OrderCreationPayload, OrderUid};
use std::{convert::Infallible, sync::Arc};
use warp::reply::with_status;
use warp::{hyper::StatusCode, Filter, Rejection};

pub fn create_order_request(
) -> impl Filter<Extract = (OrderCreationPayload,), Error = Rejection> + Clone {
    warp::path!("orders")
        .and(warp::post())
        .and(extract_payload())
}

impl IntoWarpReply for AddOrderError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            Self::OrderValidation(err) => err.into_warp_reply(),
            Self::UnsupportedSignature => with_status(
                super::error("UnsupportedSignature", "signing scheme is not supported"),
                StatusCode::BAD_REQUEST,
            ),
            Self::DuplicatedOrder => with_status(
                super::error("DuplicatedOrder", "order already exists"),
                StatusCode::BAD_REQUEST,
            ),
            Self::Database(err) => with_status(
                super::internal_error(anyhow::Error::new(err).context("create_order")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

pub fn create_order_response(result: Result<OrderUid, AddOrderError>) -> super::ApiReply {
    match result {
        Ok(uid) => with_status(warp::reply::json(&uid), StatusCode::CREATED),
        Err(err) => err.into_warp_reply(),
    }
}

pub fn create_order(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    create_order_request().and_then(move |order_payload: OrderCreationPayload| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.add_order(order_payload).await;
            if let Ok(order_uid) = result {
                tracing::debug!("order created with uid {}", order_uid);
            }
            Result::<_, Infallible>::Ok(create_order_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use model::order::{OrderCreationPayload, OrderUid};
    use serde_json::json;
    use warp::{test::request, Reply};

    #[tokio::test]
    async fn create_order_request_ok() {
        let filter = create_order_request();
        let order_payload = OrderCreationPayload::default();
        let request = request()
            .path("/orders")
            .method("POST")
            .header("content-type", "application/json")
            .json(&order_payload);
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result, order_payload);
    }

    #[tokio::test]
    async fn create_order_response_created() {
        let uid = OrderUid([1u8; 56]);
        let response = create_order_response(Ok(uid)).into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected= json!(
            "0x0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"
        );
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn create_order_response_duplicate() {
        let response = create_order_response(Err(AddOrderError::DuplicatedOrder)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_error =
            json!({"errorType": "DuplicatedOrder", "description": "order already exists"});
        assert_eq!(body, expected_error);
    }
}
