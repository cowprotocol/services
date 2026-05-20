//! Axum extractors that emit a `warn` log when request deserialization
//! fails, then delegate to the stock extractor's rejection so the HTTP
//! response shape is unchanged.

use {
    axum::{
        extract::{
            FromRequest,
            FromRequestParts,
            Request,
            rejection::{JsonRejection, QueryRejection},
        },
        http::request::Parts,
    },
    serde::de::DeserializeOwned,
};

/// JSON extractor that wraps Axum's native one and logs deserialization
/// errors.
pub struct LoggingJson<T>(pub T);

impl<S, T> FromRequest<S> for LoggingJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = JsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => {
                tracing::warn!(
                    err = %rejection,
                    target = std::any::type_name::<T>(),
                    "failed to deserialize JSON request body",
                );
                Err(rejection)
            }
        }
    }
}

/// Query extractor that wraps Axum's native one and logs deserialization
/// errors.
pub struct LoggingQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for LoggingQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = QueryRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(value)) => Ok(Self(value)),
            Err(rejection) => {
                tracing::warn!(
                    err = %rejection,
                    target = std::any::type_name::<T>(),
                    query = parts.uri.query().unwrap_or_default(),
                    "failed to deserialize query string",
                );
                Err(rejection)
            }
        }
    }
}
