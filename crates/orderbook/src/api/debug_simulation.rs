use {
    crate::{api::AppState, dto::OrderSimulationRequest, orderbook::OrderSimulationError},
    axum::{
        Json,
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    model::order::OrderUid,
    serde::Deserialize,
    std::sync::Arc,
};

#[derive(Deserialize)]
pub struct SimulationQuery {
    pub block_number: Option<u64>,
}

pub async fn debug_simulation_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
    Query(params): Query<SimulationQuery>,
) -> Response {
    match state
        .orderbook
        .simulate_order(&uid, params.block_number)
        .await
    {
        Ok(Some(result)) => (StatusCode::OK, Json(result)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "order not found"),
        )
            .into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn debug_simulation_post_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OrderSimulationRequest>,
) -> Response {
    match state.orderbook.simulate_custom_order(request).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(err) => err.into_response(),
    }
}

impl IntoResponse for OrderSimulationError {
    fn into_response(self) -> Response {
        match self {
            OrderSimulationError::NotEnabled => (
                StatusCode::METHOD_NOT_ALLOWED,
                super::error(
                    "MethodNotAllowed",
                    "order simulation endpoint is not enabled",
                ),
            )
                .into_response(),
            OrderSimulationError::MalformedInput(err) => {
                tracing::warn!(?err, "failed to parse order simulation input");
                (
                    StatusCode::BAD_REQUEST,
                    super::error("BadRequest", "malformed input"),
                )
                    .into_response()
            }
            OrderSimulationError::Other(err) => {
                tracing::error!(?err, "failed to create simulation for order");
                crate::api::internal_error_reply()
            }
        }
    }
}
