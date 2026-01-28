use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json},
    },
    model::quote::NativeTokenPrice,
    std::sync::Arc,
};

pub async fn get_native_price_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<Address>,
) -> impl IntoResponse {
    let result = state
        .native_price_estimator
        .estimate_native_price(token, state.quote_timeout)
        .await;
    match result {
        Ok(price) => (StatusCode::OK, Json(NativeTokenPrice { price })).into_response(),
        Err(err) => super::PriceEstimationErrorWrapper(err).into_response(),
    }
}
