use {
    crate::infra::{Ethereum, api::error::Error},
    axum::Json,
    serde::{Deserialize, Serialize},
    tracing::instrument,
};

pub(in crate::infra::api) fn gasprice(app: axum::Router<Ethereum>) -> axum::Router<Ethereum> {
    app.route("/gasprice", axum::routing::get(route))
}

/// Gas price components in EIP-1559 format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasPriceResponse {
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
}

#[instrument(skip(eth))]
async fn route(
    eth: axum::extract::State<Ethereum>,
) -> Result<Json<GasPriceResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    // For simplicity we use the default time limit (None)
    let gas_price = eth.gas_price().await?;

    Ok(Json(GasPriceResponse {
        max_fee_per_gas: gas_price.max_fee_per_gas,
        max_priority_fee_per_gas: gas_price.max_priority_fee_per_gas,
    }))
}
