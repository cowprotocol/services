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
    base64::Engine,
    cow_settlement_interface::{
        data::intent::{EncodedOrderIntent, OrderKind as IntentOrderKind},
        instruction::{InstructionInputParsing, create_order::CreateOrderInput},
        pda::order::find_order_pda,
    },
    database::solana::OrderKind,
    serde::Deserialize,
    solana_sdk::{clock::MAX_PROCESSING_AGE, transaction::VersionedTransaction},
};

/// Request body: the user's partially signed `CreateOrder` transaction,
/// base64-encoded. Signed by the owner, with the configured funder as its
/// unsigned fee payer.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Params {
    pub transaction: String,
}

fn bad(error_type: &'static str, description: impl Into<String>) -> error::Reply {
    error::reply(StatusCode::BAD_REQUEST, error_type, description)
}

/// Handle `POST /api/v1/orders`: validate the transaction, derive the order
/// from it, and persist both. Answers the order uid.
pub async fn create_order(
    state: axum::extract::State<State>,
    Json(params): Json<Params>,
) -> Result<(StatusCode, Json<String>), error::Reply> {
    let Some(sponsoring) = state.sponsoring() else {
        return Err(bad(
            "SponsoringDisabled",
            "sponsored order placement is not enabled on this deployment",
        ));
    };
    let bytes = base64::prelude::BASE64_STANDARD
        .decode(&params.transaction)
        .map_err(|_| bad("InvalidTransaction", "the transaction is not valid base64"))?;
    let transaction: VersionedTransaction = bincode::deserialize(&bytes).map_err(|_| {
        bad(
            "InvalidTransaction",
            "the bytes do not decode to a transaction",
        )
    })?;
    let mut order = validate(sponsoring, &transaction)?;
    order.presigned_transaction = bytes;

    // The countersign re-checks freshness, so the stored expiry only has to
    // be an upper bound: the tip cannot have moved past the blockhash's own
    // last valid height by more than the maximum age.
    let blockhash = transaction.message.recent_blockhash();
    let valid = sponsoring
        .rpc
        .is_blockhash_valid(blockhash)
        .await
        .map_err(|err| {
            tracing::error!(?err, "blockhash validity check failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?;
    if !valid {
        return Err(bad(
            "BlockhashExpired",
            "the transaction's blockhash is no longer valid, sign a fresh one",
        ));
    }
    let height = sponsoring.rpc.block_height().await.map_err(|err| {
        tracing::error!(?err, "block height fetch failed");
        error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
    })?;
    order.last_valid_block_height = u64::from(height) + MAX_PROCESSING_AGE as u64;

    let uid = order.uid;
    if let Err(err) = db::insert_sponsored_order(state.pool(), &order).await {
        let duplicate = err.downcast_ref::<sqlx::Error>().is_some_and(|err| {
            err.as_database_error()
                .and_then(|db| db.code())
                .is_some_and(|code| code == "23505")
        });
        if duplicate {
            return Err(bad("DuplicatedOrder", "an order with this uid exists"));
        }
        tracing::error!(?err, "sponsored order insert failed");
        return Err(error::reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            "",
        ));
    }
    Ok((StatusCode::CREATED, Json(const_hex::encode_prefixed(uid))))
}

/// Check the transaction is exactly the sponsored-creation shape and derive
/// the order from it. The expiry and transaction bytes are filled by the
/// caller.
fn validate(
    sponsoring: &Sponsoring,
    transaction: &VersionedTransaction,
) -> Result<db::SponsoredOrder, error::Reply> {
    let message = &transaction.message;
    if message
        .address_table_lookups()
        .is_some_and(|lookups| !lookups.is_empty())
    {
        return Err(bad(
            "InvalidTransaction",
            "a creation transaction must not use address lookup tables",
        ));
    }
    let keys = message.static_account_keys();
    if keys.first() != Some(&sponsoring.funder) {
        return Err(bad(
            "WrongFeePayer",
            "the fee payer must be the configured funder account",
        ));
    }
    let [instruction] = message.instructions() else {
        return Err(bad(
            "InvalidTransaction",
            "the transaction must carry exactly one instruction",
        ));
    };
    if keys.get(usize::from(instruction.program_id_index)) != Some(&sponsoring.settlement_program) {
        return Err(bad(
            "InvalidTransaction",
            "the instruction does not target the settlement program",
        ));
    }
    let accounts: Vec<_> = instruction
        .accounts
        .iter()
        .filter_map(|&index| keys.get(usize::from(index)).copied())
        .collect();
    if accounts.len() != instruction.accounts.len() {
        return Err(bad(
            "InvalidTransaction",
            "an account index is out of range",
        ));
    }
    let input = CreateOrderInput::parse(&instruction.data, &accounts)
        .map_err(|_| bad("InvalidTransaction", "not a CreateOrder instruction"))?;
    if *input.created_by != sponsoring.funder {
        return Err(bad(
            "WrongRentPayer",
            "the rent payer must be the configured funder account",
        ));
    }
    let (intent, uid) = EncodedOrderIntent::decode_and_hash(&input.intent_bytes)
        .map_err(|_| bad("InvalidTransaction", "the intent bytes do not decode"))?;
    if !intent.flags.created_on_chain {
        return Err(bad(
            "InvalidIntentFlags",
            "a sponsored order must be flagged created_on_chain",
        ));
    }
    let order_pda = find_order_pda(&sponsoring.settlement_program, &uid).0;
    if *input.order_pda != order_pda {
        return Err(bad(
            "WrongOrderPda",
            "the order PDA does not match the intent",
        ));
    }
    if i64::from(intent.valid_to) <= chrono::Utc::now().timestamp() {
        return Err(bad("OrderExpired", "validTo lies in the past"));
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
            return Err(bad(
                "MissingSignature",
                "a required signer other than the funder has not signed",
            ));
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
