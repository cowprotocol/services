use {
    crate::{api::AppState, orderbook::OrderSimulationError},
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    serde::Serialize,
    simulator::tenderly::dto,
    std::sync::Arc,
};

#[derive(Serialize)]
struct OrderSimulation {
    tenderly_request: dto::Request,
    error: Option<String>,
}

impl From<crate::orderbook::OrderSimulation> for OrderSimulation {
    fn from(value: crate::orderbook::OrderSimulation) -> Self {
        Self {
            tenderly_request: value.tenderly_request,
            error: value.error.map(|err| err.to_string()),
        }
    }
}

pub async fn debug_simulation_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
) -> Response {
    match state.orderbook.simulate_order(&uid).await {
        Ok(Some(result)) => (StatusCode::OK, Json(OrderSimulation::from(result))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "order not found"),
        )
            .into_response(),
        Err(OrderSimulationError::NotEnabled) => (
            StatusCode::NOT_IMPLEMENTED,
            super::error("NotImplemented", "order simulation endpoint is not enabled"),
        )
            .into_response(),
        Err(OrderSimulationError::Other(err)) => {
            tracing::error!(?err, "failed to create simulation for order");
            crate::api::internal_error_reply()
        }
    }
}
