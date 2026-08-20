//! The order endpoint: one order with its fill state by uid.

pub mod dto;

use {
    crate::infra::{api::State, db},
    axum::{Json, extract::Path, http::StatusCode},
    std::time::{SystemTime, UNIX_EPOCH},
};

/// Handle `GET /api/v1/orders/{uid}`. The uid is the order's 32-byte intent
/// hash as `0x`-prefixed hex.
pub async fn order(
    state: axum::extract::State<State>,
    Path(uid): Path<String>,
) -> Result<Json<dto::Order>, StatusCode> {
    let uid = parse_uid(&uid).ok_or(StatusCode::BAD_REQUEST)?;
    let row = db::order_by_uid(state.pool(), uid)
        .await
        .map_err(|err| {
            tracing::error!(?err, "order lookup failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(dto::Order::new(row, now_unix())))
}

fn parse_uid(uid: &str) -> Option<[u8; 32]> {
    const_hex::decode_to_array(uid).ok()
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after the unix epoch")
        .as_secs()
        .try_into()
        .expect("unix seconds fit i64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prefixed_and_bare_hex_uids() {
        let hex = "11".repeat(32);
        assert_eq!(parse_uid(&format!("0x{hex}")), Some([0x11; 32]));
        assert_eq!(parse_uid(&hex), Some([0x11; 32]));
        assert_eq!(parse_uid("0xzz"), None);
        assert_eq!(parse_uid("0x1234"), None);
    }
}
