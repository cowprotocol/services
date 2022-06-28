use crate::orderbook::{OrderCancellationError, Orderbook};
use anyhow::Result;
use model::{
    order::{OrderCancellation, OrderUid},
    signature::{EcdsaSignature, EcdsaSigningScheme},
};
use serde::{Deserialize, Serialize};
use shared::api::{convert_json_response, extract_payload, IntoWarpReply};
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply::with_status, Filter, Rejection};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CancellationPayload {
    signature: EcdsaSignature,
    signing_scheme: EcdsaSigningScheme,
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

impl IntoWarpReply for OrderCancellationError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            Self::InvalidSignature => with_status(
                super::error("InvalidSignature", "Malformed signature"),
                StatusCode::BAD_REQUEST,
            ),
            Self::AlreadyCancelled => with_status(
                super::error("AlreadyCancelled", "Order is already cancelled"),
                StatusCode::BAD_REQUEST,
            ),
            Self::OrderFullyExecuted => with_status(
                super::error("OrderFullyExecuted", "Order is fully executed"),
                StatusCode::BAD_REQUEST,
            ),
            Self::OrderExpired => with_status(
                super::error("OrderExpired", "Order is expired"),
                StatusCode::BAD_REQUEST,
            ),
            Self::OrderNotFound => with_status(
                super::error("OrderNotFound", "Order not located in database"),
                StatusCode::NOT_FOUND,
            ),
            Self::WrongOwner => with_status(
                super::error(
                    "WrongOwner",
                    "Signature recovery's owner doesn't match order's",
                ),
                StatusCode::UNAUTHORIZED,
            ),
            Self::OnChainOrder => with_status(
                super::error("OnChainOrder", "On-chain orders must be cancelled on-chain"),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => with_status(
                super::internal_error(err.context("cancel_order")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

pub fn cancel_order_response(result: Result<(), OrderCancellationError>) -> super::ApiReply {
    convert_json_response(result.map(|_| "Cancelled"))
}

pub fn cancel_order(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    cancel_order_request().and_then(move |order| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.cancel_order(order).await;
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
    use warp::{test::request, Reply};

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
                signature: EcdsaSignature {
                    r: H256(hex!(
                        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                    )),
                    s: H256(hex!(
                        "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"
                    )),
                    v: 27,
                },
                signing_scheme: EcdsaSigningScheme::Eip712,
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

    #[test]
    fn cancel_order_response_ok() {
        let response = cancel_order_response(Ok(())).into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn cancel_order_response_err() {
        let response =
            cancel_order_response(Err(OrderCancellationError::InvalidSignature)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response =
            cancel_order_response(Err(OrderCancellationError::OrderFullyExecuted)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response =
            cancel_order_response(Err(OrderCancellationError::AlreadyCancelled)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response =
            cancel_order_response(Err(OrderCancellationError::OrderExpired)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response =
            cancel_order_response(Err(OrderCancellationError::WrongOwner)).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response =
            cancel_order_response(Err(OrderCancellationError::OrderNotFound)).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = cancel_order_response(Err(OrderCancellationError::Other(
            anyhow::Error::msg("test error"),
        )))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
