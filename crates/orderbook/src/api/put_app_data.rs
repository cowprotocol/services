use {
    crate::api::{AppState, internal_error_reply},
    anyhow::Result,
    app_data::{AppDataDocument, AppDataHash},
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    std::sync::Arc,
};

pub async fn put_app_data_without_hash(
    State(state): State<Arc<AppState>>,
    Json(document): Json<AppDataDocument>,
) -> Response {
    let result = state
        .app_data
        .register(None, document.full_app_data.as_bytes())
        .await;
    response(result)
}

pub async fn put_app_data_with_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<AppDataHash>,
    Json(document): Json<AppDataDocument>,
) -> Response {
    let result = state
        .app_data
        .register(Some(hash), document.full_app_data.as_bytes())
        .await;
    response(result)
}

fn response(
    result: Result<(crate::app_data::Registered, AppDataHash), crate::app_data::RegisterError>,
) -> Response {
    match result {
        Ok((registered, hash)) => {
            let status = match registered {
                crate::app_data::Registered::New => StatusCode::CREATED,
                crate::app_data::Registered::AlreadyExisted => StatusCode::OK,
            };
            (status, Json(hash)).into_response()
        }
        Err(err) => err.into_response(),
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
