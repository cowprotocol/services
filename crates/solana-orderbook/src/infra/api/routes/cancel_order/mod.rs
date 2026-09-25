//! The off-chain cancellation endpoint: a pending sponsored order, one placed
//! through this API whose creation has not landed, stops being auctioned and
//! countersigned and reports as cancelled. Nothing is broadcast. An order with
//! a PDA on chain is cancelled through the settlement program's `CancelOrder`
//! instruction instead.

use {
    crate::infra::{
        api::{State, error, extract},
        db,
    },
    axum::{Json, http::StatusCode},
    serde::Deserialize,
    serde_with::{base64::Base64, serde_as},
    solana_sdk::signature::Signature,
};

/// The cancellation request.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// The owner's Ed25519 signature over [`message`], base64.
    #[serde_as(as = "Base64")]
    pub signature: Vec<u8>,
}

/// The bytes the owner signs to cancel an order: the UTF-8 text
/// `cancel order <uid>` with the uid as 0x-prefixed lowercase hex, the shape a
/// wallet's plain `signMessage` produces.
pub fn message(uid: [u8; 32]) -> String {
    format!("cancel order 0x{}", const_hex::encode(uid))
}

/// Handle `DELETE /api/v1/orders/{uid}`.
pub async fn cancel_order(
    state: axum::extract::State<State>,
    extract::PathUid(uid): extract::PathUid,
    extract::Json(params): extract::Json<Params>,
) -> Result<Json<&'static str>, error::Reply> {
    let internal = |err: anyhow::Error| {
        tracing::error!(?err, "order cancellation failed");
        error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
    };
    let row = db::find_order_by_uid(state.pool(), uid)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            error::reply(
                StatusCode::NOT_FOUND,
                "OrderNotFound",
                "Order was not found",
            )
        })?;
    let verified = Signature::try_from(params.signature.as_slice())
        .is_ok_and(|signature| signature.verify(row.owner.0.as_ref(), message(uid).as_bytes()));
    if !verified {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidSignature",
            "the signature does not verify against the order owner",
        ));
    }
    match db::cancel_order(state.pool(), uid)
        .await
        .map_err(internal)?
    {
        db::Cancellation::Cancelled | db::Cancellation::AlreadyCancelled => Ok(Json("Cancelled")),
        db::Cancellation::OnChain => Err(error::reply(
            StatusCode::BAD_REQUEST,
            "OnChainOrder",
            "the order exists on chain, cancel it through the settlement program",
        )),
    }
}
