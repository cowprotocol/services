use {
    crate::{
        domain::{competition, competition::order},
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

pub(in crate::infra::api) fn reveal(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/reveal", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
) -> Result<axum::Json<Solution>, (hyper::StatusCode, axum::Json<Error>)> {
    let competition = state.competition();
    let auction_id = competition.auction_id().map(|id| id.0);
    let handle_request = async {
        observe::revealing();
        let result = competition.reveal().await;
        observe::revealed(state.solver().name(), &result);
        let result = result?;
        Ok(axum::Json(Solution::new(result)))
    };

    handle_request
        .instrument(tracing::info_span!("/reveal", solver = %state.solver().name(), auction_id))
        .await
}

impl Solution {
    pub fn new(reveal: competition::Revealed) -> Self {
        Self {
            orders: reveal.orders.into_iter().map(Into::into).collect(),
            calldata: Calldata {
                internalized: reveal.internalized_calldata.into(),
                uninternalized: reveal.uninternalized_calldata.into(),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Calldata {
    #[serde_as(as = "serialize::Hex")]
    internalized: Vec<u8>,
    #[serde_as(as = "serialize::Hex")]
    uninternalized: Vec<u8>,
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    #[serde_as(as = "Vec<serialize::Hex>")]
    orders: Vec<[u8; order::UID_LEN]>,
    calldata: Calldata,
}
