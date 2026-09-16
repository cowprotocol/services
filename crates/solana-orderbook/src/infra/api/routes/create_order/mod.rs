//! The sponsored order placement endpoint: a partially signed `CreateOrder`
//! transaction comes in and the order it carries becomes placeable. Every
//! order field derives from the transaction itself, so the stored order and
//! the transaction that will create it on chain cannot disagree.

use {
    crate::infra::{
        api::{Sponsoring, State, error},
        db,
    },
    axum::{Json, http::StatusCode},
    cow_settlement_interface::{
        data::intent::{EncodedOrderIntent, OrderKind as IntentOrderKind},
        instruction::{InstructionInputParsing, create_order::CreateOrderInput},
        pda::order::find_order_pda,
    },
    database::solana::OrderKind,
    serde::Deserialize,
    serde_with::{base64::Base64, serde_as},
    solana_sdk::{clock::MAX_PROCESSING_AGE, transaction::VersionedTransaction},
};

/// Request body: the user's partially signed `CreateOrder` transaction,
/// base64-encoded on the wire. Signed by the owner, with the configured
/// funder as its unsigned fee payer.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Params {
    #[serde_as(as = "Base64")]
    pub transaction: Vec<u8>,
}

/// Rejections of a sponsored order placement. The names follow the EVM
/// orderbook's where an equivalent exists, so clients can reuse their
/// handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlacementError {
    SponsoringDisabled,
    InvalidTransaction(&'static str),
    WrongFeePayer,
    WrongRentPayer,
    InvalidIntentFlags,
    WrongOrderPda,
    InsufficientValidTo,
    InvalidSignature,
    BlockhashExpired,
    DuplicatedOrder,
}

impl From<PlacementError> for error::Reply {
    fn from(error: PlacementError) -> Self {
        let (error_type, description) = match error {
            PlacementError::SponsoringDisabled => (
                "SponsoringDisabled",
                "sponsored order placement is not enabled on this deployment",
            ),
            PlacementError::InvalidTransaction(description) => ("InvalidTransaction", description),
            PlacementError::WrongFeePayer => (
                "WrongFeePayer",
                "the fee payer must be the configured funder account",
            ),
            PlacementError::WrongRentPayer => (
                "WrongRentPayer",
                "the rent payer must be the configured funder account",
            ),
            PlacementError::InvalidIntentFlags => (
                "InvalidIntentFlags",
                "a sponsored order must be flagged created_on_chain",
            ),
            PlacementError::WrongOrderPda => {
                ("WrongOrderPda", "the order PDA does not match the intent")
            }
            PlacementError::InsufficientValidTo => {
                ("InsufficientValidTo", "validTo lies in the past")
            }
            PlacementError::InvalidSignature => (
                "InvalidSignature",
                "a required signer other than the funder has not signed",
            ),
            PlacementError::BlockhashExpired => (
                "BlockhashExpired",
                "the transaction's blockhash is no longer valid, sign a fresh one",
            ),
            PlacementError::DuplicatedOrder => ("DuplicatedOrder", "an order with this uid exists"),
        };
        error::reply(StatusCode::BAD_REQUEST, error_type, description)
    }
}

fn internal_error_reply(err: impl std::fmt::Debug, what: &str) -> error::Reply {
    tracing::error!(?err, "{what}");
    error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
}

/// Handle `POST /api/v1/orders`: validate the transaction, derive the order
/// from it, and persist both. Answers the order uid like the EVM orderbook.
pub async fn create_order(
    state: axum::extract::State<State>,
    Json(params): Json<Params>,
) -> Result<(StatusCode, Json<String>), error::Reply> {
    let Some(sponsoring) = state.sponsoring() else {
        return Err(PlacementError::SponsoringDisabled.into());
    };
    let transaction: VersionedTransaction =
        bincode::deserialize(&params.transaction).map_err(|_| {
            PlacementError::InvalidTransaction("the bytes do not decode to a transaction")
        })?;
    let mut order = validate(sponsoring, &transaction)?;
    order.presigned_transaction = params.transaction;

    // Short-circuit replays before the RPC probes. The insert's unique
    // violation below stays as the race-safe backstop.
    let duplicate = db::order_exists(state.pool(), &order.uid)
        .await
        .map_err(|err| internal_error_reply(err, "order existence check failed"))?;
    if duplicate {
        return Err(PlacementError::DuplicatedOrder.into());
    }

    // The countersign re-checks freshness, so the stored expiry only has to
    // be an upper bound: the tip cannot have moved past the blockhash's own
    // last valid height by more than the maximum age.
    let blockhash = transaction.message.recent_blockhash();
    let valid = sponsoring
        .rpc
        .is_blockhash_valid(blockhash)
        .await
        .map_err(|err| internal_error_reply(err, "blockhash validity check failed"))?;
    if !valid {
        return Err(PlacementError::BlockhashExpired.into());
    }
    let height = sponsoring
        .rpc
        .block_height()
        .await
        .map_err(|err| internal_error_reply(err, "block height fetch failed"))?;
    order.last_valid_block_height = u64::from(height) + MAX_PROCESSING_AGE as u64;

    let uid = order.uid;
    if let Err(err) = db::insert_sponsored_order(state.pool(), &order).await {
        let duplicate = err
            .downcast_ref::<sqlx::Error>()
            .and_then(|err| err.as_database_error())
            .is_some_and(|db| db.is_unique_violation());
        if duplicate {
            return Err(PlacementError::DuplicatedOrder.into());
        }
        return Err(internal_error_reply(err, "sponsored order insert failed"));
    }
    Ok((StatusCode::CREATED, Json(const_hex::encode_prefixed(uid))))
}

/// Check the transaction is exactly the sponsored-creation shape and derive
/// the order from it. The expiry and transaction bytes are filled by the
/// caller.
fn validate(
    sponsoring: &Sponsoring,
    transaction: &VersionedTransaction,
) -> Result<db::SponsoredOrder, PlacementError> {
    let message = &transaction.message;
    if message
        .address_table_lookups()
        .is_some_and(|lookups| !lookups.is_empty())
    {
        return Err(PlacementError::InvalidTransaction(
            "a creation transaction must not use address lookup tables",
        ));
    }
    let keys = message.static_account_keys();
    if keys.first() != Some(&sponsoring.funder) {
        return Err(PlacementError::WrongFeePayer);
    }
    // TODO(BE-277): accept the whitelisted bundle template (wrap SOL, approve
    // the delegate, create the destination account) in front of `CreateOrder`.
    // A lone `CreateOrder` fits only traders whose accounts are already set
    // up, so this gate has to fall before the frontend integrates.
    let [instruction] = message.instructions() else {
        return Err(PlacementError::InvalidTransaction(
            "the transaction must carry exactly one instruction",
        ));
    };
    if keys.get(usize::from(instruction.program_id_index)) != Some(&sponsoring.settlement_program) {
        return Err(PlacementError::InvalidTransaction(
            "the instruction does not target the settlement program",
        ));
    }
    let accounts: Vec<_> = instruction
        .accounts
        .iter()
        .filter_map(|&index| keys.get(usize::from(index)).copied())
        .collect();
    if accounts.len() != instruction.accounts.len() {
        return Err(PlacementError::InvalidTransaction(
            "an account index is out of range",
        ));
    }
    let input = CreateOrderInput::parse(&instruction.data, &accounts)
        .map_err(|_| PlacementError::InvalidTransaction("not a CreateOrder instruction"))?;
    if *input.created_by != sponsoring.funder {
        return Err(PlacementError::WrongRentPayer);
    }
    let (intent, uid) = EncodedOrderIntent::decode_and_hash(&input.intent_bytes)
        .map_err(|_| PlacementError::InvalidTransaction("the intent bytes do not decode"))?;
    if !intent.flags.created_on_chain {
        return Err(PlacementError::InvalidIntentFlags);
    }
    let order_pda = find_order_pda(&sponsoring.settlement_program, &uid).0;
    if *input.order_pda != order_pda {
        return Err(PlacementError::WrongOrderPda);
    }
    if i64::from(intent.valid_to) <= chrono::Utc::now().timestamp() {
        return Err(PlacementError::InsufficientValidTo);
    }

    // Every required signer except the funder must have signed: the funder's
    // slot stays a placeholder until the autopilot countersigns.
    let message_bytes = message.serialize();
    for (index, key) in keys
        .iter()
        .take(usize::from(message.header().num_required_signatures))
        .enumerate()
    {
        if *key == sponsoring.funder {
            continue;
        }
        let signed = transaction
            .signatures
            .get(index)
            .is_some_and(|signature| signature.verify(key.as_ref(), &message_bytes));
        if !signed {
            return Err(PlacementError::InvalidSignature);
        }
    }

    Ok(db::SponsoredOrder {
        uid: uid.to_bytes(),
        owner: intent.owner.to_bytes(),
        sell_token: intent.sell_mint.to_bytes(),
        buy_token: intent.buy_mint.to_bytes(),
        sell_token_account: intent.sell_token_account.to_bytes(),
        buy_token_account: intent.buy_token_account.to_bytes(),
        sell_amount: intent.sell_amount,
        buy_amount: intent.buy_amount,
        valid_to: intent.valid_to,
        kind: match intent.flags.kind {
            IntentOrderKind::Sell => OrderKind::Sell,
            IntentOrderKind::Buy => OrderKind::Buy,
        },
        partially_fillable: intent.flags.partially_fillable,
        app_data: intent.app_data,
        order_pda: order_pda.to_bytes(),
        presigned_transaction: Vec::new(),
        last_valid_block_height: 0,
    })
}
