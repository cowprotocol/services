use crate::infra::api::{Error, State};

mod dto;

pub use dto::AuctionError;

pub(in crate::infra::api) fn solve(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/solve", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    auction: axum::Json<dto::Auction>,
) -> Result<axum::Json<dto::Solution>, axum::Json<Error>> {
    let auction = auction
        .0
        .into_domain(state.liquidity(), state.now())
        .await?;
    let competition = state.competition();
    let (solution_id, score) = competition.solve(&auction).await?;
    Ok(axum::Json(dto::Solution::from_domain(solution_id, score)))
}
