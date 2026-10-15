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
    // Temporary staging knob: take part in one solve out of every N so other
    // solvers win settlements to test against. Counts the solves this driver
    // receives, since the auction id is a timestamp that would alias the
    // stride.
    if let Some(stride) = state.solve_every_nth_auction() {
        let seq = state.next_solve_seq();
        if !seq.is_multiple_of(stride.get()) {
            tracing::debug!(%auction_id, %stride, seq, "sitting out the auction");
            return Ok(Json(dto::SolveResponse::new(Vec::new())));
        }
    }
    let solutions = state
        .competition()
        .solve(auction_id, auction)
        .instrument(tracing::info_span!("/solve", solver = %state.competition().solver_name(), auction_id = %auction_id))
        .await?;
    Ok(Json(dto::SolveResponse::new(solutions)))
}
