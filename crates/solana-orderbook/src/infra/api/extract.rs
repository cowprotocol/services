//! Request extractors with rejections in the API's error shape.

use {
    super::error,
    axum::{
        extract::{FromRequest, FromRequestParts, Request},
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

/// A JSON body. Rejects a malformed or incomplete body in the API's error
/// shape, which `axum::Json` answers with plain text instead.
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = error::Reply;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|axum::Json(body)| Self(body))
            .map_err(|rejection| {
                tracing::debug!(%rejection, "rejected request body");
                error::reply(
                    StatusCode::BAD_REQUEST,
                    "InvalidRequestBody",
                    "The request body could not be parsed.",
                )
            })
    }
}
