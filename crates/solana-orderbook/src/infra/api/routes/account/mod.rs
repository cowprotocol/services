//! The account orders endpoint: one owner's orders, paginated.

use {
    super::order::{dto, now_unix},
    crate::infra::{
        api::{State, error},
        db,
    },
    axum::{
        Json,
        extract::{Path, Query},
        http::StatusCode,
    },
    serde::Deserialize,
    solana_sdk::pubkey::Pubkey,
    std::str::FromStr,
};

const DEFAULT_OFFSET: u64 = 0;
const DEFAULT_LIMIT: u64 = 10;
const MIN_LIMIT: u64 = 1;
const MAX_LIMIT: u64 = 1000;

/// Pagination parameters, with the EVM orderbook's defaults and bounds. The
/// unsigned types reject negative values at deserialization, as on EVM.
#[derive(Debug, Deserialize)]
pub struct Params {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

/// Handle `GET /api/v1/account/{owner}/orders`: the owner's orders with
/// their fill state, newest first.
pub async fn account_orders(
    state: axum::extract::State<State>,
    Path(owner): Path<String>,
    Query(params): Query<Params>,
) -> Result<Json<Vec<dto::Order>>, error::Reply> {
    let owner = Pubkey::from_str(&owner).map_err(|_| {
        error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidOwner",
            "owner must be a base58-encoded public key",
        )
    })?;
    let offset = params.offset.unwrap_or(DEFAULT_OFFSET);
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if !(MIN_LIMIT..=MAX_LIMIT).contains(&limit) {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "LIMIT_OUT_OF_BOUNDS",
            "The pagination limit is [1,1000].",
        ));
    }
    // The limit is bounded above, and an offset past i64::MAX addresses no
    // conceivable row.
    let offset = i64::try_from(offset).unwrap_or(i64::MAX);
    let limit = i64::try_from(limit).expect("limit is at most 1000");

    let rows = db::orders_by_owner(state.pool(), owner.to_bytes(), offset, limit)
        .await
        .map_err(|err| {
            tracing::error!(?err, "account orders lookup failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?;
    let now = now_unix();
    Ok(Json(
        rows.into_iter()
            .map(|row| dto::Order::new(row, now))
            .collect(),
    ))
}
