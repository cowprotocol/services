use {
    crate::{
        domain::eth,
        infra::{Ethereum, api::error::Error},
        util::serialize,
    },
    axum::Json,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    tracing::instrument,
};

pub(in crate::infra::api) fn gasprice(app: axum::Router<Ethereum>) -> axum::Router<Ethereum> {
    app.route("/gasprice", axum::routing::get(route))
}

/// Gas price components in EIP-1559 format.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasPriceResponse {
    #[serde_as(as = "serialize::U256")]
    pub max_fee_per_gas: eth::U256,
    #[serde_as(as = "serialize::U256")]
    pub max_priority_fee_per_gas: eth::U256,
    #[serde_as(as = "serialize::U256")]
    pub base_fee_per_gas: eth::U256,
}

#[instrument(skip(eth))]
async fn route(
    eth: axum::extract::State<Ethereum>,
) -> Result<Json<GasPriceResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    // For simplicity we use the default time limit (None)
    let gas_price = eth.gas_price().await?;

    Ok(Json(GasPriceResponse {
        max_fee_per_gas: gas_price.max().0.0,
        max_priority_fee_per_gas: gas_price.tip().0.0,
        base_fee_per_gas: gas_price.base().0.0,
    }))
}
