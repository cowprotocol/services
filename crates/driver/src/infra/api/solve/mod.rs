mod dto;

pub(super) fn route(router: axum::Router<super::State>) -> axum::Router<super::State> {
    router.route("/solve", axum::routing::post(solve))
}

async fn solve(
    state: axum::extract::State<super::State>,
    auction: axum::extract::Json<dto::Auction>,
) -> axum::response::Json<dto::Solution> {
    // TODO Report errors instead of unwrapping
    let auction = auction.0.into_domain(state.now()).unwrap();
    let competition = state.competition();
    let (solution_id, score) = competition.solve(&auction).await.unwrap();
    axum::response::Json(dto::Solution::from_domain(solution_id, score))
}
