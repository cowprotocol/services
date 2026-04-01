use {
    crate::{api::AppState, orderbook::OrderSimulationError},
    alloy::primitives::Address,
    axum::{
        Json,
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    model::order::{
        BuyTokenDestination,
        Interactions,
        Order,
        OrderData,
        OrderKind,
        OrderMetadata,
        OrderUid,
        SellTokenSource,
    },
    number::serialization::HexOrDecimalU256,
    serde::Deserialize,
    serde_with::serde_as,
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

/// Request body for the POST /api/v1/debug/simulation endpoint.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationRequest {
    pub sell_token: Address,
    pub buy_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: alloy::primitives::U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: alloy::primitives::U256,
    pub kind: OrderKind,
    pub owner: Address,
    #[serde(default)]
    pub receiver: Option<Address>,
    #[serde(default)]
    pub sell_token_balance: SellTokenSource,
    #[serde(default)]
    pub buy_token_balance: BuyTokenDestination,
    /// Full app data JSON. Defaults to `"{}"` if omitted.
    #[serde(default)]
    pub app_data: Option<String>,
    #[serde(default)]
    pub interactions: Interactions,
    #[serde(default)]
    pub block_number: Option<u64>,
}

pub async fn debug_simulation_post_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SimulationRequest>,
) -> Response {
    let order = Order {
        metadata: OrderMetadata {
            owner: request.owner,
            full_app_data: Some(request.app_data.unwrap_or_else(|| "{}".to_owned())),
            ..Default::default()
        },
        data: OrderData {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            sell_amount: request.sell_amount,
            buy_amount: request.buy_amount,
            kind: request.kind,
            receiver: request.receiver,
            sell_token_balance: request.sell_token_balance,
            buy_token_balance: request.buy_token_balance,
            ..Default::default()
        },
        interactions: request.interactions,
        ..Default::default()
    };

    match state
        .orderbook
        .simulate_custom_order(order, request.block_number)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
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
