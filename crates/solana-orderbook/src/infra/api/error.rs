//! API error responses.

use {
    axum::{Json, http::StatusCode},
    serde::Serialize,
};

/// Error body: a machine-readable type and a human-readable description.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub error_type: &'static str,
    pub description: String,
}

/// An error response: the status code and the error body.
pub type Reply = (StatusCode, Json<Error>);

/// Build an error response.
pub fn reply(
    status: StatusCode,
    error_type: &'static str,
    description: impl Into<String>,
) -> Reply {
    (
        status,
        Json(Error {
            error_type,
            description: description.into(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_is_stable() {
        let (status, body) = reply(StatusCode::BAD_REQUEST, "InvalidTradeFilter", "why");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            serde_json::to_value(&body.0).unwrap(),
            serde_json::json!({"errorType": "InvalidTradeFilter", "description": "why"})
        );
    }
}
