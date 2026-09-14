pub mod dto;

pub use dto::AuctionError;
use {
    crate::infra::api::{LoggingJson, State, error::Error as ApiError},
    axum::{Json, http::StatusCode},
    tracing::Instrument,
};

/// Handle `POST /solve`: parse the autopilot's auction, send it to this
/// solver engine, and answer with the converted solutions.
pub(crate) async fn solve(
    state: axum::extract::State<State>,
    LoggingJson(request): LoggingJson<dto::SolveRequest>,
) -> Result<Json<dto::SolveResponse>, (StatusCode, Json<ApiError>)> {
    let auction = request.into_domain()?;
    let auction_id = auction.id.ok_or(dto::AuctionError::InvalidAuctionId)?;
    // Temporary staging knob: sit out auctions off the configured stride so
    // other solvers win settlements to test against.
    if let Some(stride) = state.solve_every_nth_auction() {
        let id = u64::try_from(auction_id.get()).expect("auction ids are positive");
        if id % stride.get() != 0 {
            tracing::debug!(%auction_id, %stride, "sitting out the auction");
            return Ok(Json(dto::SolveResponse::new(Vec::new())));
        }
    }
    let solutions = state
        .competition()
        .solve(auction_id, &auction)
        .instrument(tracing::info_span!("/solve", solver = %state.competition().solver_name(), auction_id = %auction_id))
        .await?;
    Ok(Json(dto::SolveResponse::new(solutions)))
}
