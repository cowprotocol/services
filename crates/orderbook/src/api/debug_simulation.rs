use {
    crate::{api::AppState, dto::OrderSimulationRequest, orderbook::OrderSimulationError},
    alloy::primitives::{Address, U256},
    axum::{
        Json,
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    model::order::OrderUid,
    number::serialization::HexOrDecimalU256,
    serde::Deserialize,
    serde_with::serde_as,
    std::sync::Arc,
};
#[serde_as]
#[derive(Deserialize)]
pub struct SimulationQuery {
    pub block_number: Option<u64>,
    /// Override for how much of the order has already been filled, expressed
    /// in the order's fill token (sell token for sell orders, buy token for
    /// buy orders). When absent, the current on-chain fill state from the
    /// order metadata is used.
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub executed_amount: Option<U256>,
}

pub async fn debug_simulation_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
    Query(params): Query<SimulationQuery>,
) -> Response {
    match state
        .orderbook
        .simulate_order(&uid, params.block_number, params.executed_amount)
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
