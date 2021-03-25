use crate::api::extract_payload;
use crate::orderbook::{OrderCancellationResult, Orderbook};
use anyhow::Result;
use model::order::{OrderCancellation, OrderUid};
use model::Signature;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, Filter, Rejection, Reply};

#[derive(Deserialize, Serialize)]
struct CancellationPayload {
    signature: Signature,
}

pub fn cancel_order_request(
) -> impl Filter<Extract = (OrderCancellation,), Error = Rejection> + Clone {
    warp::path!("orders" / OrderUid)
        .and(warp::delete())
        .and(extract_payload())
        .map(|uid, payload: CancellationPayload| OrderCancellation {
            order_uid: uid,
            signature: payload.signature,
        })
}

pub fn cancel_order_response(result: Result<OrderCancellationResult>) -> impl Reply {
    let (body, status_code) = match result {
        Ok(OrderCancellationResult::Cancelled) => (warp::reply::json(&"Cancelled"), StatusCode::OK),
        Ok(OrderCancellationResult::InvalidSignature) => (
            super::error("InvalidSignature", "Likely malformed signature"),
            StatusCode::BAD_REQUEST,
        ),
        Ok(OrderCancellationResult::OrderNotFound) => (
            super::error("OrderNotFound", "order not located in database"),
            StatusCode::NOT_FOUND,
        ),
        Ok(OrderCancellationResult::WrongOwner) => (
            super::error(
                "WrongOwner",
                "Signature recovery's owner doesn't match order's",
            ),
            StatusCode::UNAUTHORIZED,
        ),
        Err(_) => (super::internal_error(), StatusCode::INTERNAL_SERVER_ERROR),
    };
    warp::reply::with_status(body, status_code)
}

pub fn cancel_order(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    cancel_order_request().and_then(move |order| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.cancel_order(order).await;
            if let Err(err) = &result {
                tracing::error!(?err, ?order, "cancel_order error");
            }
            Result::<_, Infallible>::Ok(cancel_order_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;

    #[tokio::test]
    async fn cancel_order_request_ok() {
        let filter = cancel_order_request();
        let cancellation = OrderCancellation::default();

        let request = request()
            .path(&format!("/orders/{:}", cancellation.order_uid))
            .method("DELETE")
            .header("content-type", "application/json")
            .json(&CancellationPayload {
                signature: cancellation.signature,
            });
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result, cancellation);
    }

    #[tokio::test]
    async fn cancel_order_response_ok() {
        let response =
            cancel_order_response(Ok(OrderCancellationResult::Cancelled)).into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cancel_order_response_err() {
        let response =
            cancel_order_response(Ok(OrderCancellationResult::InvalidSignature)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response =
            cancel_order_response(Ok(OrderCancellationResult::WrongOwner)).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response =
            cancel_order_response(Ok(OrderCancellationResult::OrderNotFound)).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = cancel_order_response(Err(anyhow::Error::msg("test error"))).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
