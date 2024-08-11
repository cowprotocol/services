use {
    super::with_status,
    axum::{http::StatusCode, routing::MethodRouter},
    primitive_types::H160,
    serde_json::json,
    shared::api::{internal_error_reply, ApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/users/:user/total_surplus";
async fn handler(
    state: axum::extract::State<super::State>,
    user: axum::extract::Path<H160>,
) -> ApiReply {
    let surplus = state.database.total_surplus(&user).await;
    match surplus {
        Ok(surplus) => with_status(
            json!({
                "totalSurplus": surplus.to_string()
            }),
            StatusCode::OK,
        ),
        Err(err) => {
            tracing::error!(?err, ?user, "failed to compute total surplus");
            internal_error_reply()
        }
    }
}
