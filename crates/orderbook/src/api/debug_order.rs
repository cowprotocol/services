use {
    crate::api::AppState,
    axum::{
        extract::{Path, State},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, sync::Arc},
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugOrderRequest {
    #[serde(default)]
    pub create_tenderly_simulation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugOrderResponse {
    pub simulation_succeeded: bool,
    pub gas_estimate: Option<u64>,
    pub revert_reason: Option<String>,
    pub tenderly_url: Option<String>,
    pub from: String,
    pub call_target: String,
    pub used_wrapper: bool,
    pub block_number: u64,
    pub calldata: String,
    pub state_overrides: serde_json::Value,
}

pub async fn debug_order_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
    headers: HeaderMap,
    body: Option<Json<DebugOrderRequest>>,
) -> Response {
    // Check if debug endpoint is enabled (auth tokens configured)
    if state.debug_route_auth_tokens.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "debug endpoint is not enabled"),
        )
            .into_response();
    }

    // Auth check
    let token_name = match authenticate(&headers, &state.debug_route_auth_tokens) {
        Some(name) => name,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                super::error("Unauthorized", "invalid or missing x-auth-token"),
            )
                .into_response();
        }
    };

    tracing::info!(%uid, token_name, "debug simulation requested");

    let order = match state.orderbook.get_order(&uid).await {
        Ok(Some(order)) => order,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                super::error("NotFound", "order not found"),
            )
                .into_response();
        }
        Err(err) => {
            tracing::error!(?err, "failed to fetch order for debug simulation");
            return crate::api::internal_error_reply();
        }
    };

    let request = body.map(|b| b.0).unwrap_or_default();

    match state
        .debug_simulator
        .simulate(&order, request.create_tenderly_simulation)
        .await
    {
        Ok(result) => {
            let overrides_json = serde_json::to_value(&result.state_overrides).unwrap_or_default();
            let response = DebugOrderResponse {
                simulation_succeeded: result.succeeded,
                gas_estimate: result.gas_estimate,
                revert_reason: result.revert_reason,
                tenderly_url: result.tenderly_url,
                from: format!("{:#x}", result.from),
                call_target: format!("{:#x}", result.call_target),
                used_wrapper: result.used_wrapper,
                block_number: result.block_number,
                calldata: format!("0x{}", const_hex::encode(&result.calldata)),
                state_overrides: overrides_json,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            tracing::error!(?err, "debug simulation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                super::error("SimulationFailed", format!("{err:#}")),
            )
                .into_response()
        }
    }
}

/// Returns the token name if the x-auth-token header matches a configured
/// token. The map is keyed by secret → name.
fn authenticate<'a>(headers: &HeaderMap, tokens: &'a HashMap<String, String>) -> Option<&'a str> {
    let header_value = headers.get("x-auth-token")?.to_str().ok()?;
    tokens.get(header_value).map(String::as_str)
}
