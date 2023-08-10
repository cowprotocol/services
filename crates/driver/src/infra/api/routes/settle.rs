use {
    crate::{
        domain::competition,
        infra::{
            api::{Error, State},
            observe,
        },
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
    tracing::Instrument,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
) -> Result<axum::Json<Calldata>, (hyper::StatusCode, axum::Json<Error>)> {
    let competition = state.competition();
    let auction_id = competition.auction_id().map(|id| id.0);
    let handle_request = async {
        observe::settling();
        let result = competition.settle().await;
        observe::settled(state.solver().name(), &result);
        let calldata = result?;
        Ok(axum::Json(Calldata::new(calldata)))
    };

    handle_request
        .instrument(tracing::info_span!("/settle", solver = %state.solver().name(), auction_id))
        .await
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Calldata {
    calldata: CalldataInner,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalldataInner {
    #[serde_as(as = "serialize::Hex")]
    internalized: Vec<u8>,
    #[serde_as(as = "serialize::Hex")]
    uninternalized: Vec<u8>,
}

impl Calldata {
    pub fn new(calldata: competition::Settled) -> Self {
        Self {
            calldata: CalldataInner {
                internalized: calldata.internalized_calldata.into(),
                uninternalized: calldata.uninternalized_calldata.into(),
            },
        }
    }
}
