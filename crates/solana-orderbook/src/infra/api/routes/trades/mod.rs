//! The trades endpoint: fills by order uid or owner.

pub mod dto;

use {
    crate::infra::{api::State, db},
    axum::{Json, extract::Query, http::StatusCode},
    serde::Deserialize,
    solana_sdk::pubkey::Pubkey,
    std::str::FromStr,
};

/// Query parameters: exactly one of `orderUid` or `owner`, the EVM
/// orderbook's contract.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    pub order_uid: Option<String>,
    pub owner: Option<String>,
}

/// Handle `GET /api/v1/trades`.
pub async fn trades(
    state: axum::extract::State<State>,
    Query(params): Query<Params>,
) -> Result<Json<Vec<dto::Trade>>, StatusCode> {
    let (order_uid, owner) = match (&params.order_uid, &params.owner) {
        (Some(uid), None) => (
            Some(const_hex::decode_to_array(uid).map_err(|_| StatusCode::BAD_REQUEST)?),
            None,
        ),
        (None, Some(owner)) => (
            None,
            Some(
                Pubkey::from_str(owner)
                    .map_err(|_| StatusCode::BAD_REQUEST)?
                    .to_bytes(),
            ),
        ),
        // Exactly one filter, the EVM orderbook's contract.
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let rows = db::trades(state.pool(), order_uid, owner)
        .await
        .map_err(|err| {
            tracing::error!(?err, "trades lookup failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(rows.into_iter().map(dto::Trade::from).collect()))
}
