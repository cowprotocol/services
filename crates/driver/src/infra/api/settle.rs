pub(super) fn route(router: axum::Router<super::State>) -> axum::Router<super::State> {
    router.route("/settle/:solution_id", axum::routing::post(settle))
}

async fn settle(
    state: axum::extract::State<super::State>,
    axum::extract::Path(solution_id): axum::extract::Path<u64>,
) {
    // TODO Report errors instead of unwrapping
    let competition = state.competition();
    competition.settle(solution_id.into()).await.unwrap();
}
