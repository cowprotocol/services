//! Axum extractors that emit a `warn` log when request deserialization
//! fails, then delegate to the stock extractor's rejection so the HTTP
//! response shape is unchanged.

use {
    axum::{
        body::Body,
        extract::{FromRequest, Request},
    },
    serde::de::DeserializeOwned,
};

/// JSON extractor that wraps Axum's native one and logs deserialization
/// errors before returning the same rejection.
pub struct LoggingJson<T>(pub T);

impl<S, T> FromRequest<S> for LoggingJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = axum::extract::rejection::JsonRejection;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => {
                tracing::warn!(
                    err = %rejection,
                    request_type = std::any::type_name::<T>(),
                    "failed to deserialize JSON request body",
                );
                Err(rejection)
            }
        }
    }
}
