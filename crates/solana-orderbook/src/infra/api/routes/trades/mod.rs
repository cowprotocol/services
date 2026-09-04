//! The trades endpoint: fills by order uid or owner.

pub mod dto;

use {
    crate::infra::{
        api::{State, error},
        db,
    },
    axum::{Json, extract::Query, http::StatusCode},
    serde::Deserialize,
    solana_sdk::pubkey::Pubkey,
    std::str::FromStr,
};

/// Query parameters. Exactly one of `orderUid` or `owner` must be set.
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
) -> Result<Json<Vec<dto::Trade>>, error::Reply> {
    let (order_uid, owner) = match (&params.order_uid, &params.owner) {
        (Some(uid), None) => (
            Some(const_hex::decode_to_array(uid).map_err(|_| {
                error::reply(
                    StatusCode::BAD_REQUEST,
                    "InvalidOrderUid",
                    "orderUid must be 32 bytes of hex",
                )
            })?),
            None,
        ),
        (None, Some(owner)) => (
            None,
            Some(
                Pubkey::from_str(owner)
                    .map_err(|_| {
                        error::reply(
                            StatusCode::BAD_REQUEST,
                            "InvalidOwner",
                            "owner must be a base58-encoded public key",
                        )
                    })?
                    .to_bytes(),
            ),
        ),
        _ => {
            return Err(error::reply(
                StatusCode::BAD_REQUEST,
                "InvalidTradeFilter",
                "Must specify exactly one of owner or orderUid.",
            ));
        }
    };
    let rows = db::trades(state.pool(), order_uid, owner)
        .await
        .map_err(|err| {
            tracing::error!(?err, "trades lookup failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?;
    Ok(Json(rows.into_iter().map(dto::Trade::from).collect()))
}
