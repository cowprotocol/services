use {
    crate::{api::AppState, orderbook::OrderSimulationError},
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    std::sync::Arc,
};

pub async fn debug_simulation_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
) -> Response {
    match state.orderbook.simulate_order(&uid).await {
        Ok(Some(result)) => (StatusCode::OK, Json(result)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "order not found"),
        )
            .into_response(),
        Err(OrderSimulationError::NotEnabled) => (
            StatusCode::METHOD_NOT_ALLOWED,
            super::error(
                "MethodNotAllowed",
                "order simulation endpoint is not enabled",
            ),
        )
            .into_response(),
        Err(OrderSimulationError::Other(err)) => {
            tracing::error!(?err, "failed to create simulation for order");
            crate::api::internal_error_reply()
        }
    }
}
