use {
    super::with_status,
    anyhow::Result,
    app_data::{AppDataDocument, AppDataHash},
    axum::{http::StatusCode, routing::MethodRouter},
    shared::api::{error, internal_error_reply, ApiReply, IntoApiReply},
};

pub fn without_hash_route() -> (&'static str, MethodRouter<super::State>) {
    (
        WITHOUT_HASH_ENDPOINT,
        axum::routing::put(without_hash_handler),
    )
}

const WITHOUT_HASH_ENDPOINT: &str = "/api/v1/app_data";
// TODO double check that we can indeed omit the app data hash in the path
// and figure out how we limit the request body size
async fn without_hash_handler(
    state: axum::extract::State<super::State>,
    document: axum::extract::Json<AppDataDocument>,
) -> ApiReply {
    let result = state
        .app_data
        .register(None, document.0.full_app_data.as_bytes())
        .await;
    response(result)
}

pub fn with_hash_route() -> (&'static str, MethodRouter<super::State>) {
    (WITH_HASH_ENDPOINT, axum::routing::put(with_hash_handler))
}

const WITH_HASH_ENDPOINT: &str = "/api/v1/app_data/:app_data_hash";
// TODO double check that we can indeed omit the app data hash in the path
// and figure out how we limit the request body size
async fn with_hash_handler(
    state: axum::extract::State<super::State>,
    hash: axum::extract::Path<AppDataHash>,
    document: axum::extract::Json<AppDataDocument>,
) -> ApiReply {
    let result = state
        .app_data
        .register(Some(hash.0), document.0.full_app_data.as_bytes())
        .await;
    response(result)
}

fn response(
    result: Result<(crate::app_data::Registered, AppDataHash), crate::app_data::RegisterError>,
) -> super::ApiReply {
    match result {
        Ok((registered, hash)) => {
            let status = match registered {
                crate::app_data::Registered::New => StatusCode::CREATED,
                crate::app_data::Registered::AlreadyExisted => StatusCode::OK,
            };
            with_status(serde_json::to_value(hash).unwrap(), status)
        }
        Err(err) => err.into_api_reply(),
    }
}

impl IntoApiReply for crate::app_data::RegisterError {
    fn into_api_reply(self) -> super::ApiReply {
        match self {
            Self::Invalid(err) => with_status(
                error("AppDataInvalid", err.to_string()),
                StatusCode::BAD_REQUEST,
            ),
            err @ Self::HashMismatch { .. } => with_status(
                error("AppDataHashMismatch", err.to_string()),
                StatusCode::BAD_REQUEST,
            ),
            err @ Self::DataMismatch { .. } => with_status(
                error("AppDataMismatch", err.to_string()),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => {
                tracing::error!(?err, "app_data::SaveError::Other");
                internal_error_reply()
            }
        }
    }
}
