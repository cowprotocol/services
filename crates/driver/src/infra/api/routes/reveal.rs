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
};

pub(in crate::infra::api) fn reveal(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/reveal", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
) -> Result<axum::Json<Solution>, (hyper::StatusCode, axum::Json<Error>)> {
    let competition = state.competition();
    observe::revealing(state.solver().name(), competition.auction_id());
    let result = competition.reveal().await;
    observe::revealed(state.solver().name(), competition.auction_id(), &result);
    let result = result?;
    Ok(axum::Json(Solution::new(result)))
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
