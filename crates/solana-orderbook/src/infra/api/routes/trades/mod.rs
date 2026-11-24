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
    std::{collections::HashMap, str::FromStr},
};

const DEFAULT_OFFSET: u64 = 0;
const DEFAULT_LIMIT: u64 = 10;
const MIN_LIMIT: u64 = 1;
const MAX_LIMIT: u64 = 1000;

/// Query parameters. Exactly one of `orderUid` or `owner` must be set. The
/// pagination defaults and bounds are the EVM orderbook's, and the unsigned
/// types reject negative values at deserialization, as on EVM.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    pub order_uid: Option<String>,
    pub owner: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

/// Handle `GET /api/v2/trades`.
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
    let offset = params.offset.unwrap_or(DEFAULT_OFFSET);
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if !(MIN_LIMIT..=MAX_LIMIT).contains(&limit) {
        // The EVM v2 trades error, verbatim.
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidLimit",
            format!("limit must be between {MIN_LIMIT} and {MAX_LIMIT}"),
        ));
    }
    // The limit is bounded above, and an offset past i64::MAX addresses no
    // conceivable row.
    let offset = i64::try_from(offset).unwrap_or(i64::MAX);
    let limit = i64::try_from(limit).expect("limit is at most 1000");
    let trades = async {
        let rows = db::get_trades(state.pool(), order_uid, owner, offset, limit).await?;
        let executions: Vec<_> = rows
            .iter()
            .filter_map(|row| Some((row.auction_id?, row.order_uid)))
            .collect();
        let mut fees: HashMap<_, Vec<dto::ExecutedProtocolFee>> = HashMap::new();
        for fee in db::executed_protocol_fees(state.pool(), &executions).await? {
            fees.entry((fee.auction_id, fee.order_uid))
                .or_default()
                .push(dto::ExecutedProtocolFee::try_from(fee)?);
        }
        anyhow::Ok(
            rows.into_iter()
                .map(|row| {
                    let fees = row
                        .auction_id
                        .and_then(|auction_id| fees.get(&(auction_id, row.order_uid)).cloned())
                        .unwrap_or_default();
                    dto::Trade::new(row, fees)
                })
                .collect::<Vec<_>>(),
        )
    }
    .await
    .map_err(|err| {
        tracing::error!(?err, "trades lookup failed");
        error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
    })?;
    Ok(Json(trades))
}
