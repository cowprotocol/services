use {
    crate::api::{AppState, PriceEstimationErrorWrapper},
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::quote::NativeTokenPrice,
    std::{str::FromStr, sync::Arc},
};

pub async fn get_native_price_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(token) = Address::from_str(&token) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    state
        .native_price_estimator
        .estimate_native_price(token, state.quote_timeout)
        .await
        .map(|price| Json(NativeTokenPrice { price }))
        .map_err(PriceEstimationErrorWrapper)
        .into_response()
}
