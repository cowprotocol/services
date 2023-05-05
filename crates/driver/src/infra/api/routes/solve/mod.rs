mod dto;

pub use dto::AuctionError;
use {
    crate::infra::api::{Error, State},
    tap::TapFallible,
};

pub(in crate::infra::api) fn solve(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/solve", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    auction: axum::Json<dto::Auction>,
) -> Result<axum::Json<dto::Solution>, (hyper::StatusCode, axum::Json<Error>)> {
    let auction = auction.0.into_domain(state.eth()).await.tap_err(|err| {
        tracing::warn!(?err, "error creating auction");
    })?;
    let competition = state.competition();
    let (id, score) = competition.solve(&auction).await.tap_err(|err| {
        tracing::warn!(?err, "error solving auction");
    })?;
    Ok(axum::Json(dto::Solution::from_domain(id, score)))
}
