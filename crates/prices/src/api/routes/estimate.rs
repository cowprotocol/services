use {
    crate::{
        api::State,
        core::{self, eth},
        util::serialize,
    },
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

pub(in crate::api) fn estimate(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/estimate", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    swap: axum::Json<Swap>,
) -> Result<axum::Json<Estimate>, hyper::StatusCode> {
    let estimators = state.estimators();
    // TODO So I guess I need an ethereum node connection to get the gas price?
    // TODO I could also pass it to estimate() and fetch the gas price in parallel
    // with everything else
    core::estimate(
        swap.0.into(),
        eth::U256::from(30000000000u64).into(),
        estimators,
    )
    .await
    .map(|estimate| axum::Json(estimate.into()))
    .map_err(|_| hyper::StatusCode::BAD_REQUEST)
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Swap {
    from: eth::H160,
    to: eth::H160,
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
}

impl From<Swap> for core::Swap {
    fn from(value: Swap) -> Self {
        Self {
            from: value.from.into(),
            to: value.to.into(),
            amount: value.amount.into(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
struct Estimate {
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    fee: eth::U256,
}

impl From<core::Estimate> for Estimate {
    fn from(value: core::Estimate) -> Self {
        Self {
            amount: value.amount.into(),
            fee: value.fee.into(),
        }
    }
}
