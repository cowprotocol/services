use {
    super::with_status,
    app_data::{AppDataDocument, AppDataHash},
    axum::{http::StatusCode, routing::MethodRouter},
    shared::api::ApiReply,
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/app_data/:app_data_hash";
async fn handler(
    state: axum::extract::State<super::State>,
    contract_app_data: axum::extract::Path<AppDataHash>,
) -> ApiReply {
    let result = state.database.get_full_app_data(&contract_app_data.0).await;
    match result {
        Ok(Some(response)) => with_status(
            serde_json::to_value(&AppDataDocument {
                full_app_data: response,
            })
            .unwrap(),
            StatusCode::OK,
        ),
        Ok(None) => with_status(
            serde_json::Value::String("full app data not found".into()),
            StatusCode::NOT_FOUND,
        ),
        Err(err) => {
            tracing::error!(?err, "get_app_data_by_hash");
            shared::api::internal_error_reply()
        }
    }
}
