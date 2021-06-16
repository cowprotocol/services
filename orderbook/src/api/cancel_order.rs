use crate::api::extract_payload;
use crate::orderbook::{OrderCancellationResult, Orderbook};
use anyhow::Result;
use model::Signature;
use model::{
    order::{OrderCancellation, OrderUid},
    SigningScheme,
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, Filter, Rejection, Reply};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CancellationPayload {
    signature: Signature,
    signing_scheme: SigningScheme,
}

pub fn cancel_order_request(
) -> impl Filter<Extract = (OrderCancellation,), Error = Rejection> + Clone {
    warp::path!("orders" / OrderUid)
        .and(warp::delete())
        .and(extract_payload())
        .map(|uid, payload: CancellationPayload| OrderCancellation {
            order_uid: uid,
            signature: payload.signature,
            signing_scheme: payload.signing_scheme,
        })
}

pub fn cancel_order_response(result: Result<OrderCancellationResult>) -> impl Reply {
    let (body, status_code) = match result {
        Ok(OrderCancellationResult::Cancelled) => (warp::reply::json(&"Cancelled"), StatusCode::OK),
        Ok(OrderCancellationResult::InvalidSignature) => (
            super::error("InvalidSignature", "Likely malformed signature"),
            StatusCode::BAD_REQUEST,
        ),
        Ok(OrderCancellationResult::AlreadyCancelled) => (
            super::error("AlreadyCancelled", "Order is already cancelled"),
            StatusCode::BAD_REQUEST,
        ),
        Ok(OrderCancellationResult::OrderFullyExecuted) => (
            super::error("OrderFullyExecuted", "Order is fully executed"),
            StatusCode::BAD_REQUEST,
        ),
        Ok(OrderCancellationResult::OrderExpired) => (
            super::error("OrderExpired", "Order is expired"),
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
    use ethcontract::H256;
    use hex_literal::hex;
    use serde_json::json;
    use warp::test::request;

    #[test]
    fn cancellation_payload_deserialization() {
        assert_eq!(
            CancellationPayload::deserialize(json!({
                "signature": "0x\
                    000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\
                    202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f\
                    1b",
                "signingScheme": "eip712"
            }))
            .unwrap(),
            CancellationPayload {
                signature: Signature {
                    r: H256(hex!(
                        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                    )),
                    s: H256(hex!(
                        "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"
                    )),
                    v: 27,
                },
                signing_scheme: SigningScheme::Eip712,
            },
        );
    }

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
                signing_scheme: cancellation.signing_scheme,
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
            cancel_order_response(Ok(OrderCancellationResult::OrderFullyExecuted)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response =
            cancel_order_response(Ok(OrderCancellationResult::AlreadyCancelled)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response =
            cancel_order_response(Ok(OrderCancellationResult::OrderExpired)).into_response();
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
