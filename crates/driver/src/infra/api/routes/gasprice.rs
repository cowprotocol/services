use {
    crate::infra::{Ethereum, api::error::Error},
    alloy::eips::eip1559::Eip1559Estimation,
    axum::Json,
    tracing::instrument,
};

pub(in crate::infra::api) fn gasprice(app: axum::Router<Ethereum>) -> axum::Router<Ethereum> {
    app.route("/gasprice", axum::routing::get(route))
}

#[instrument(skip(eth))]
async fn route(
    eth: axum::extract::State<Ethereum>,
) -> Result<Json<Eip1559Estimation>, (hyper::StatusCode, axum::Json<Error>)> {
    // For simplicity we use the default time limit (None)
    let gas_price = eth.gas_price().await?;

    Ok(Json(Eip1559Estimation {
        max_fee_per_gas: gas_price.max_fee_per_gas,
        max_priority_fee_per_gas: gas_price.max_priority_fee_per_gas,
    }))
}
