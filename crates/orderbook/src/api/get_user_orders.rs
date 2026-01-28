use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Json},
    },
    serde::Deserialize,
    std::sync::Arc,
};

#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct QueryParams {
    offset: Option<u64>,
    limit: Option<u64>,
}

pub async fn get_user_orders_handler(
    State(state): State<Arc<AppState>>,
    Path(owner): Path<Address>,
    Query(query): Query<QueryParams>,
) -> impl IntoResponse {
    const DEFAULT_OFFSET: u64 = 0;
    const DEFAULT_LIMIT: u64 = 10;
    const MIN_LIMIT: u64 = 1;
    const MAX_LIMIT: u64 = 1000;

    let offset = query.offset.unwrap_or(DEFAULT_OFFSET);
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);

    if !(MIN_LIMIT..=MAX_LIMIT).contains(&limit) {
        return (
            StatusCode::BAD_REQUEST,
            super::error(
                "LIMIT_OUT_OF_BOUNDS",
                format!("The pagination limit is [{MIN_LIMIT},{MAX_LIMIT}]."),
            ),
        )
            .into_response();
    }

    let result = state.orderbook.get_user_orders(&owner, offset, limit).await;
    match result {
        Ok(reply) => (StatusCode::OK, Json(reply)).into_response(),
        Err(err) => {
            tracing::error!(?err, "get_user_orders");
            crate::api::internal_error_reply()
        }
    }
}
