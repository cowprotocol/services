//! The solver competition endpoints: one auction's competition by auction
//! id, by settlement transaction, or the latest.

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

/// Handle `GET /api/v2/solver_competition/{auction_id}`.
pub async fn solver_competition_by_id(
    state: axum::extract::State<State>,
    Path(auction_id): Path<String>,
) -> Result<Json<dto::Competition>, error::Reply> {
    let auction_id: u64 = auction_id.parse().map_err(|_| {
        error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidAuctionId",
            "auctionId must be a non-negative integer",
        )
    })?;
    // Well formed past i64::MAX, but no auction has such an id.
    let Ok(auction_id) = i64::try_from(auction_id) else {
        return reply(Ok(None));
    };
    reply(db::find_competition_by_id(state.pool(), auction_id).await)
}

/// Handle `GET /api/v2/solver_competition/by_tx_hash/{tx_hash}`. The path
/// is the one clients share across chains, the parameter is the settlement
/// transaction's signature.
pub async fn solver_competition_by_tx_hash(
    state: axum::extract::State<State>,
    Path(tx_hash): Path<String>,
) -> Result<Json<dto::Competition>, error::Reply> {
    let signature = Signature::from_str(&tx_hash).map_err(|_| {
        error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidTransactionSignature",
            "txHash must be a base58-encoded transaction signature",
        )
    })?;
    reply(db::find_competition_by_tx_signature(state.pool(), signature.into()).await)
}

/// Handle `GET /api/v2/solver_competition/latest`.
pub async fn solver_competition_latest(
    state: axum::extract::State<State>,
) -> Result<Json<dto::Competition>, error::Reply> {
    reply(db::find_latest_competition(state.pool()).await)
}

/// The response to a competition lookup: the competition, 404 without one,
/// 500 on a failed read.
fn reply(
    lookup: anyhow::Result<Option<db::Competition>>,
) -> Result<Json<dto::Competition>, error::Reply> {
    lookup
        .map_err(|err| {
            tracing::error!(?err, "solver competition lookup failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?
        .map(|competition| Json(dto::Competition::from(competition)))
        .ok_or_else(|| {
            error::reply(
                StatusCode::NOT_FOUND,
                "NotFound",
                "solver competition not found",
            )
        })
}
