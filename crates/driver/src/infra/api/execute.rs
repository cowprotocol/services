pub(super) fn route(router: axum::Router<super::State>) -> axum::Router<super::State> {
    router.route("/execute/:solution_id", axum::routing::post(execute))
}

async fn execute(
    state: axum::extract::State<super::State>,
    axum::extract::Path(solution_id): axum::extract::Path<u32>,
) {
    // TODO Report errors instead of unwrapping
    let competition = state.competition();
    competition.execute(solution_id.into()).await.unwrap();
}
