use crate::logic;

mod dto;

pub(super) fn route(app: super::Router) -> super::Router {
    app.route("/solve", axum::routing::post(solve))
}

async fn solve(
    state: axum::extract::State<super::State>,
    auction: axum::extract::Json<dto::Auction>,
) -> axum::response::Json<dto::Solution> {
    let auction: logic::competition::Auction = auction.0.into();
    // TODO Report errors instead of unwrapping
    let score = logic::competition::solve(state.solvers(), auction)
        .await
        .unwrap();
    axum::response::Json(score.into())
}
