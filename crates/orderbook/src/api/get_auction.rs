use {
    super::with_status,
    axum::{http::StatusCode, routing::MethodRouter},
    shared::api::{error, ApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/auction";
async fn handler(state: axum::extract::State<super::State>) -> ApiReply {
    let result = state.orderbook.get_auction().await;
    match result {
        Ok(Some(auction)) => with_status(serde_json::to_value(&auction).unwrap(), StatusCode::OK),
        Ok(None) => with_status(
            error("NotFound", "There is no active auction"),
            StatusCode::NOT_FOUND,
        ),
        Err(err) => {
            tracing::error!(?err, "/api/v1/get_auction");
            shared::api::internal_error_reply()
        }
    }
}
