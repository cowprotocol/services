use crate::logic::competition::{self, solution};

mod dto;

pub(super) fn route(router: axum::Router<super::State>) -> axum::Router<super::State> {
    router.route("/solve", axum::routing::post(solve))
}

async fn solve(
    state: axum::extract::State<super::State>,
    auction: axum::extract::Json<dto::Auction>,
) -> axum::response::Json<dto::Solution> {
    // TODO Report errors instead of unwrapping
    let auction = auction.0.try_into().unwrap();
    let score = competition::solve(
        state.solver(),
        state.ethereum(),
        state.simulator(),
        &auction,
    )
    .await
    .unwrap();
    axum::response::Json(dto::Solution::new(solution::Id::random(), score))
}
