//! Axum extractors that wrap the stock extractor and, when request
//! deserialization fails, respond with this API's structured error format
//! (`{ errorType, description }`) instead of the stock plain-text rejection, so
//! clients can parse every response from the API as JSON.

use {
    super::error,
    axum::{
        extract::{FromRequest, Request},
        response::{IntoResponse, Response},
    },
    serde::{Serialize, de::DeserializeOwned},
};

/// JSON extractor that wraps Axum's native one and renders deserialization
/// errors as this API's structured error response. Also serves as a response
/// type so it can fully replace [`axum::Json`] where both are used.
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => Err((
                rejection.status(),
                error("InvalidJson", rejection.body_text()),
            )
                .into_response()),
        }
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode, header::CONTENT_TYPE},
        },
        serde::Deserialize,
    };

    #[derive(Deserialize)]
    struct Dummy {
        _required: u32,
    }

    async fn structured_error(body: &'static str) -> (StatusCode, serde_json::Value) {
        let request = Request::builder()
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = match Json::<Dummy>::from_request(request, &()).await {
            Ok(_) => panic!("malformed body should have been rejected"),
            Err(response) => response,
        };

        let status = response.status();
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/json",
            "error response must be JSON"
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    // Reproduces cowprotocol/services#4439: an empty JSON object misses a
    // required field and must yield a structured JSON error, not plain text.
    #[tokio::test]
    async fn missing_field_returns_structured_json_error() {
        let (status, json) = structured_error("{}").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["errorType"], "InvalidJson");
        assert!(
            json["description"]
                .as_str()
                .unwrap()
                .contains("missing field")
        );
    }

    // Reproduces cowprotocol/services#4440: a field whose value cannot be
    // deserialized into the target type (e.g. an invalid token address) must
    // also yield a structured JSON error rather than plain text.
    #[tokio::test]
    async fn invalid_field_value_returns_structured_json_error() {
        let (status, json) = structured_error(r#"{"_required": "not-a-number"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["errorType"], "InvalidJson");
    }

    #[tokio::test]
    async fn invalid_syntax_returns_structured_json_error() {
        let (status, json) = structured_error("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["errorType"], "InvalidJson");
    }
}
