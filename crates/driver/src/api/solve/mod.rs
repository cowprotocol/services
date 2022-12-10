use crate::logic;

mod dto;

pub(super) fn route(router: axum::Router<super::State>) -> axum::Router<super::State> {
    router.route("/solve", axum::routing::post(solve))
}

async fn solve(
    state: axum::extract::State<super::State>,
    auction: axum::extract::Json<dto::Auction>,
) -> axum::response::Json<dto::Solution> {
    let auction = auction.0.into();
    // TODO Report errors instead of unwrapping
    let score = logic::competition::solve(
        state.solver(),
        state.ethereum(),
        state.simulator(),
        &auction,
    )
    .await
    .unwrap();
    axum::response::Json(score.into())
}
