use crate::infra::api::State;

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle/:solution_id", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    axum::extract::Path(solution_id): axum::extract::Path<u64>,
) {
    // TODO Report errors instead of unwrapping
    let competition = state.competition();
    competition.settle(solution_id.into()).await.unwrap();
}
