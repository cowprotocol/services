use {
    crate::api::{AppState, PriceEstimationErrorWrapper},
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        response::Json,
    },
    model::quote::NativeTokenPrice,
    std::sync::Arc,
};

pub async fn get_native_price_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<Address>,
) -> Result<Json<NativeTokenPrice>, PriceEstimationErrorWrapper> {
    state
        .native_price_estimator
        .estimate_native_price(token, state.quote_timeout)
        .await
        .map(|price| Json(NativeTokenPrice { price }))
        .map_err(PriceEstimationErrorWrapper)
}
