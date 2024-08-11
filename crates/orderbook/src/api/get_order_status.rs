use {
    super::with_status,
    axum::{http::StatusCode, routing::MethodRouter},
    model::order::OrderUid,
    shared::api::{error, ApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/orders/:uid/status";
async fn handler(
    state: axum::extract::State<super::State>,
    uid: axum::extract::Path<OrderUid>,
) -> ApiReply {
    let status = state.orderbook.get_order_status(&uid.0).await;
    match status {
        Ok(Some(status)) => with_status(serde_json::to_value(&status).unwrap(), StatusCode::OK),
        Ok(None) => with_status(
            error("OrderNotFound", "Order not located in database"),
            StatusCode::NOT_FOUND,
        ),
        Err(err) => {
            tracing::error!(?err, "get_order_status");
            shared::api::internal_error_reply()
        }
    }
}
