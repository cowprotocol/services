use crate::infra::api::State;

mod dto;

pub(in crate::infra::api) fn solve(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/solve", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    auction: axum::extract::Json<dto::Auction>,
) -> axum::response::Json<dto::Solution> {
    // TODO Report errors instead of unwrapping
    let auction = auction
        .0
        .into_domain(state.liquidity(), state.now())
        .await
        .unwrap();

    let competition = state.competition();
    let (solution_id, score) = competition.solve(&auction).await.unwrap();
    axum::response::Json(dto::Solution::from_domain(solution_id, score))
}
