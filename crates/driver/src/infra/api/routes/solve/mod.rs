mod dto;

pub use dto::AuctionError;
use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
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
        observe::invalid_dto(state.solver().name(), err, "/solve", "auction");
    })?;
    observe::auction(state.solver().name(), &auction);
    let competition = state.competition();
    let result = competition.solve(&auction).await;
    observe::solved(state.solver().name(), &auction, &result);
    Ok(axum::Json(dto::Solution::new(result?, &competition.solver)))
}
