pub mod dto;

pub use dto::AuctionError;
use {
    crate::infra::api::{State, error::Error as ApiError},
    axum::{Json, http::StatusCode},
    tracing::Instrument,
};

/// Handle `POST /solve`: parse the autopilot's auction, send it to this
/// solver engine, and answer with the converted solutions.
pub async fn solve(
    state: axum::extract::State<State>,
    Json(request): Json<dto::SolveRequest>,
) -> Result<Json<dto::SolveResponse>, (StatusCode, Json<ApiError>)> {
    let program_id = state.blockchain().program_id();
    let auction = request.into_domain()?;
    let auction_id = auction.id;
    let solutions = state
        .competition()
        .solve(&auction, program_id)
        .instrument(tracing::info_span!("/solve", auction_id = %auction_id))
        .await?;
    Ok(Json(dto::SolveResponse::new(solutions)))
}
