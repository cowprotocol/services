use crate::infra::{
    api::{Error, State},
    observe,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle/:solution_id", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    axum::extract::Path(solution_id): axum::extract::Path<u64>,
) -> Result<(), (hyper::StatusCode, axum::Json<Error>)> {
    let competition = state.competition();
    let solution_id = solution_id.into();
    observe::settling(state.solver().name(), solution_id);
    let result = competition.settle(solution_id).await;
    observe::settled(state.solver().name(), solution_id, &result);
    result.map_err(Into::into)
}
