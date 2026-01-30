use {
    crate::{api::AppState, orderbook::OrderCancellationError},
    anyhow::Result,
    axum::{
        Json,
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    model::order::{CancellationPayload, OrderCancellation, OrderUid},
    std::sync::Arc,
};

pub async fn cancel_order_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
    Json(payload): Json<CancellationPayload>,
) -> Response {
    let order_cancellation = OrderCancellation {
        order_uid: uid,
        signature: payload.signature,
        signing_scheme: payload.signing_scheme,
    };
    let result = state.orderbook.cancel_order(order_cancellation).await;
    cancel_order_response(result)
}

impl IntoResponse for OrderCancellationError {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidSignature => (
                StatusCode::BAD_REQUEST,
                super::error("InvalidSignature", "Malformed signature"),
            )
                .into_response(),
            Self::AlreadyCancelled => (
                StatusCode::BAD_REQUEST,
                super::error("AlreadyCancelled", "Order is already cancelled"),
            )
                .into_response(),
            Self::OrderFullyExecuted => (
                StatusCode::BAD_REQUEST,
                super::error("OrderFullyExecuted", "Order is fully executed"),
            )
                .into_response(),
            Self::OrderExpired => (
                StatusCode::BAD_REQUEST,
                super::error("OrderExpired", "Order is expired"),
            )
                .into_response(),
            Self::OrderNotFound => (
                StatusCode::NOT_FOUND,
                super::error("OrderNotFound", "Order not located in database"),
            )
                .into_response(),
            Self::WrongOwner => (
                StatusCode::UNAUTHORIZED,
                super::error(
                    "WrongOwner",
                    "Signature recovery's owner doesn't match order's",
                ),
            )
                .into_response(),
            Self::OnChainOrder => (
                StatusCode::BAD_REQUEST,
                super::error("OnChainOrder", "On-chain orders must be cancelled on-chain"),
            )
                .into_response(),
            Self::Other(err) => {
                tracing::error!(?err, "cancel_order");
                crate::api::internal_error_reply()
            }
        }
    }
}

pub fn cancel_order_response(result: Result<(), OrderCancellationError>) -> Response {
    match result {
        Ok(_) => (axum::http::StatusCode::OK, axum::Json("Cancelled")).into_response(),
        Err(err) => err.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::b256,
        model::signature::{EcdsaSignature, EcdsaSigningScheme},
        serde_json::json,
    };

    #[test]
    fn cancellation_payload_deserialization() {
        assert_eq!(
            serde_json::from_value::<CancellationPayload>(json!({
                "signature": "0x\
                    000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\
                    202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f\
                    1b",
                "signingScheme": "eip712"
            }))
            .unwrap(),
            CancellationPayload {
                signature: EcdsaSignature {
                    r: b256!("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"),
                    s: b256!("202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"),
                    v: 27,
                },
                signing_scheme: EcdsaSigningScheme::Eip712,
            },
        );
    }

    #[test]
    fn cancel_order_response_ok() {
        let response = cancel_order_response(Ok(()));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn cancel_order_response_err() {
        let response = cancel_order_response(Err(OrderCancellationError::InvalidSignature));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = cancel_order_response(Err(OrderCancellationError::OrderFullyExecuted));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = cancel_order_response(Err(OrderCancellationError::AlreadyCancelled));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = cancel_order_response(Err(OrderCancellationError::OrderExpired));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = cancel_order_response(Err(OrderCancellationError::WrongOwner));
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = cancel_order_response(Err(OrderCancellationError::OrderNotFound));
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = cancel_order_response(Err(OrderCancellationError::Other(
            anyhow::Error::msg("test error"),
        )));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
