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
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle/:solution_id", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    axum::extract::Path(solution_id): axum::extract::Path<u64>,
) -> Result<axum::Json<Calldata>, (hyper::StatusCode, axum::Json<Error>)> {
    let competition = state.competition();
    let solution_id = solution_id.into();
    observe::settling(state.solver().name(), solution_id);
    let result = competition.settle(solution_id).await;
    observe::settled(state.solver().name(), solution_id, &result);
    let calldata = result?;
    Ok(axum::Json(Calldata::new(calldata)))
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
    pub fn new(calldata: competition::Calldata) -> Self {
        Self {
            calldata: CalldataInner {
                internalized: calldata.internalized.into(),
                uninternalized: calldata.uninternalized.into(),
            },
        }
    }
}
