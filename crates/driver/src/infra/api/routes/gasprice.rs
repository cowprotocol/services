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

pub(in crate::infra::api) fn gasprice(
    app: axum::Router<Ethereum>,
) -> axum::Router<Ethereum> {
    app.route(
        "/gasprice",
        axum::routing::get(route),
    )
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasPriceResponse {
    /// The estimated effective gas price that will be paid for a transaction.
    #[serde_as(as = "serialize::U256")]
    pub effective_gas_price: eth::U256,
}

#[instrument(skip(eth))]
async fn route(
    eth: axum::extract::State<Ethereum>,
) -> Result<Json<GasPriceResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    // Get gas price estimation with default time limit (None uses default from config)
    let gas_price = eth
        .gas_price(None)
        .await?;

    let effective = gas_price.effective();
    
    Ok(Json(GasPriceResponse {
        effective_gas_price: effective.0.0,
    }))
}
