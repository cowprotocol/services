use {
    crate::api::{AppState, internal_error_reply},
    app_data::{AppDataDocument, AppDataHash},
    axum::{
        body::{self},
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    std::{str::FromStr, sync::Arc},
};

pub async fn put_app_data_without_hash(
    State(state): State<Arc<AppState>>,
    body: body::Bytes,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(document) = serde_json::from_slice::<AppDataDocument>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    state
        .app_data
        .register(None, document.full_app_data.as_bytes())
        .await
        .into_response()
}

pub async fn put_app_data_with_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    body: body::Bytes,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(hash) = AppDataHash::from_str(&hash) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(document) = serde_json::from_slice::<AppDataDocument>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    state
        .app_data
        .register(Some(hash), document.full_app_data.as_bytes())
        .await
        .into_response()
}

impl IntoResponse for crate::app_data::Register {
    fn into_response(self) -> Response {
        let status = match self.status {
            crate::app_data::RegistrationStatus::New => StatusCode::CREATED,
            crate::app_data::RegistrationStatus::AlreadyExisted => StatusCode::OK,
        };
        (status, Json(self.hash)).into_response()
    }
}

impl IntoResponse for crate::app_data::RegisterError {
    fn into_response(self) -> Response {
        match self {
            Self::Invalid(err) => (
                StatusCode::BAD_REQUEST,
                super::error("AppDataInvalid", err.to_string()),
            )
                .into_response(),
            err @ Self::HashMismatch { .. } => (
                StatusCode::BAD_REQUEST,
                super::error("AppDataHashMismatch", err.to_string()),
            )
                .into_response(),
            err @ Self::DataMismatch { .. } => (
                StatusCode::BAD_REQUEST,
                super::error("AppDataMismatch", err.to_string()),
            )
                .into_response(),
            Self::Other(err) => {
                tracing::error!(?err, "app_data::SaveError::Other");
                internal_error_reply()
            }
        }
    }
}
