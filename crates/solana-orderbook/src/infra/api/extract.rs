//! Request extractors with rejections in the API's error shape.

use {
    super::error,
    axum::{
        extract::FromRequestParts,
        http::{StatusCode, request::Parts},
    },
};

/// The order uid path parameter: the order's 32-byte intent hash as hex.
pub struct PathUid(pub [u8; 32]);

impl<S: Send + Sync> FromRequestParts<S> for PathUid {
    type Rejection = error::Reply;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let invalid = || {
            error::reply(
                StatusCode::BAD_REQUEST,
                "InvalidOrderUid",
                "orderUid must be 32 bytes of hex",
            )
        };
        let axum::extract::Path(raw) =
            axum::extract::Path::<String>::from_request_parts(parts, state)
                .await
                .map_err(|_| invalid())?;
        const_hex::decode_to_array(&raw)
            .map(Self)
            .map_err(|_| invalid())
    }
}
