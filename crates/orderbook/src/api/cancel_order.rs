use {
    super::with_status,
    crate::orderbook::OrderCancellationError,
    axum::{http::StatusCode, response::IntoResponse, routing::MethodRouter},
    model::order::{CancellationPayload, OrderCancellation, OrderUid},
    shared::api::{convert_json_response, error, ApiReply, IntoApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::delete(handler))
}

const ENDPOINT: &str = "/api/v1/orders/:uid";
async fn handler(
    state: axum::extract::State<super::State>,
    order_uid: axum::extract::Path<OrderUid>,
    payload: axum::extract::Json<CancellationPayload>,
) -> impl IntoResponse {
    let request = OrderCancellation {
        order_uid: *order_uid,
        signature: payload.signature,
        signing_scheme: payload.signing_scheme,
    };
    let result = state.0.orderbook.cancel_order(request).await;
    convert_json_response(result.map(|_| "Cancelled"))
}

impl IntoApiReply for OrderCancellationError {
    fn into_api_reply(self) -> ApiReply {
        match self {
            Self::InvalidSignature => with_status(
                error("InvalidSignature", "Malformed signature"),
                StatusCode::BAD_REQUEST,
            ),
            Self::AlreadyCancelled => with_status(
                error("AlreadyCancelled", "Order is already cancelled"),
                StatusCode::BAD_REQUEST,
            ),
            Self::OrderFullyExecuted => with_status(
                error("OrderFullyExecuted", "Order is fully executed"),
                StatusCode::BAD_REQUEST,
            ),
            Self::OrderExpired => with_status(
                error("OrderExpired", "Order is expired"),
                StatusCode::BAD_REQUEST,
            ),
            Self::OrderNotFound => with_status(
                error("OrderNotFound", "Order not located in database"),
                StatusCode::NOT_FOUND,
            ),
            Self::WrongOwner => with_status(
                error(
                    "WrongOwner",
                    "Signature recovery's owner doesn't match order's",
                ),
                StatusCode::UNAUTHORIZED,
            ),
            Self::OnChainOrder => with_status(
                error("OnChainOrder", "On-chain orders must be cancelled on-chain"),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => {
                tracing::error!(?err, "cancel_order");
                shared::api::internal_error_reply()
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use {
//         super::*,
//         ethcontract::H256,
//         hex_literal::hex,
//         model::signature::{EcdsaSignature, EcdsaSigningScheme},
//         serde_json::json,
//         warp::{test::request, Reply},
//     };

//     #[test]
//     fn cancellation_payload_deserialization() {
//         assert_eq!(
//             serde_json::from_value::<CancellationPayload>(json!({
//                 "signature": "0x\
//
// 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\
// 202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f\
// 1b",                 "signingScheme": "eip712"
//             }))
//             .unwrap(),
//             CancellationPayload {
//                 signature: EcdsaSignature {
//                     r: H256(hex!(
//
// "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
//                     )),
//                     s: H256(hex!(
//
// "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"
//                     )),
//                     v: 27,
//                 },
//                 signing_scheme: EcdsaSigningScheme::Eip712,
//             },
//         );
//     }

//     #[tokio::test]
//     async fn cancel_order_request_ok() {
//         let filter = cancel_order_request();
//         let cancellation = OrderCancellation::default();

//         let request = request()
//             .path(&format!("/v1/orders/{}", cancellation.order_uid))
//             .method("DELETE")
//             .header("content-type", "application/json")
//             .json(&CancellationPayload {
//                 signature: cancellation.signature,
//                 signing_scheme: cancellation.signing_scheme,
//             });
//         let result = request.filter(&filter).await.unwrap();
//         assert_eq!(result, cancellation);
//     }

//     #[test]
//     fn cancel_order_response_ok() {
//         let response = cancel_order_response(Ok(())).into_response();
//         assert_eq!(response.status(), StatusCode::OK);
//     }

//     #[test]
//     fn cancel_order_response_err() {
//         let response =
//
// cancel_order_response(Err(OrderCancellationError::InvalidSignature)).
// into_response();         assert_eq!(response.status(),
// StatusCode::BAD_REQUEST);

//         let response =
//
// cancel_order_response(Err(OrderCancellationError::OrderFullyExecuted)).
// into_response();         assert_eq!(response.status(),
// StatusCode::BAD_REQUEST);

//         let response =
//
// cancel_order_response(Err(OrderCancellationError::AlreadyCancelled)).
// into_response();         assert_eq!(response.status(),
// StatusCode::BAD_REQUEST);

//         let response =
//
// cancel_order_response(Err(OrderCancellationError::OrderExpired)).
// into_response();         assert_eq!(response.status(),
// StatusCode::BAD_REQUEST);

//         let response =
//
// cancel_order_response(Err(OrderCancellationError::WrongOwner)).
// into_response();         assert_eq!(response.status(),
// StatusCode::UNAUTHORIZED);

//         let response =
//
// cancel_order_response(Err(OrderCancellationError::OrderNotFound)).
// into_response();         assert_eq!(response.status(),
// StatusCode::NOT_FOUND);

//         let response =
// cancel_order_response(Err(OrderCancellationError::Other(
// anyhow::Error::msg("test error"),         )))
//         .into_response();
//         assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
//     }
// }
