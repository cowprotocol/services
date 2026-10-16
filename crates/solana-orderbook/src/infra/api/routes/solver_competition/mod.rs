//! The solver competition endpoints: every solution proposed for one auction,
//! by auction id or by a settlement transaction of the auction.

pub mod dto;

use {
    crate::infra::{
        api::{State, error},
        db,
    },
    axum::{Json, extract::Path, http::StatusCode},
    solana_sdk::signature::Signature,
    std::str::FromStr,
};

fn internal(err: anyhow::Error) -> error::Reply {
    tracing::error!(?err, "solver competition lookup failed");
    error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
}

fn not_found() -> error::Reply {
    error::reply(
        StatusCode::NOT_FOUND,
        "NotFound",
        "Solver competition was not found",
    )
}

async fn load(state: &State, auction_id: i64) -> Result<Json<dto::Competition>, error::Reply> {
    db::load_competition(state.pool(), auction_id)
        .await
        .map_err(internal)?
        .map(|rows| Json(dto::Competition::from(rows)))
        .ok_or_else(not_found)
}

/// Handle `GET /api/v2/solver_competition/{auction_id}`.
pub async fn solver_competition(
    state: axum::extract::State<State>,
    Path(auction_id): Path<String>,
) -> Result<Json<dto::Competition>, error::Reply> {
    // Unsigned so a negative id is a bad request, as on EVM. An id past
    // i64::MAX is well formed but names no auction.
    let auction_id = u64::from_str(&auction_id).map_err(|_| {
        error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidAuctionId",
            "auctionId must be a non-negative integer",
        )
    })?;
    let auction_id = i64::try_from(auction_id).map_err(|_| not_found())?;
    load(&state, auction_id).await
}

/// Handle `GET /api/v2/solver_competition/by_tx_hash/{tx_signature}`. The
/// path keeps the EVM name, the parameter is a Solana signature.
pub async fn solver_competition_by_tx(
    state: axum::extract::State<State>,
    Path(tx_signature): Path<String>,
) -> Result<Json<dto::Competition>, error::Reply> {
    let tx_signature = Signature::from_str(&tx_signature).map_err(|_| {
        error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidTxSignature",
            "txSignature must be a base58-encoded transaction signature",
        )
    })?;
    let auction_id = db::find_settlement_auction(state.pool(), tx_signature.into())
        .await
        .map_err(internal)?
        .ok_or_else(not_found)?;
    load(&state, auction_id).await
}
