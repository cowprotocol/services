//! The sponsored order placement endpoint: a partially signed creation
//! transaction comes in, and every order field derives from it, so the
//! stored order and the transaction creating it on chain cannot disagree.
//!
//! The funder countersigns as fee payer, so only the whitelisted
//! preparation steps may precede the mandatory trailing `CreateOrder`: wrap
//! SOL, delegate the sell account, create the buy token account. The
//! buy-account creation is required even when the account exists (it is
//! idempotent on chain): settlement pays out to it and never creates it,
//! and the instruction proves receivability without a lookup or a race.

use {
    crate::infra::{
        api::{Sponsoring, State, error},
        db,
    },
    axum::{Json, http::StatusCode},
    bigdecimal::BigDecimal,
    cow_settlement_interface::{
        data::intent::{EncodedOrderIntent, OrderIntent, OrderKind as IntentOrderKind},
        instruction::{InstructionInputParsing, create_order::CreateOrderInput},
        pda::{order::find_order_pda, state::find_state_pda},
    },
    database::solana::OrderKind,
    serde::Deserialize,
    serde_with::{base64::Base64, serde_as},
    solana_sdk::{
        clock::MAX_PROCESSING_AGE,
        message::compiled_instruction::CompiledInstruction,
        pubkey::Pubkey,
        transaction::VersionedTransaction,
    },
    solana_system_interface::instruction::SystemInstruction,
    spl_token_interface::instruction::TokenInstruction,
};

/// Request body: the user's partially signed creation transaction,
/// base64-encoded on the wire. Signed by the owner, with the configured
/// funder as its unsigned fee payer.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Params {
    #[serde_as(as = "Base64")]
    pub transaction: Vec<u8>,
    /// The id the quote endpoint answered for this order, if any.
    #[serde(default)]
    pub quote_id: Option<i64>,
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
    WrongDelegate,
    SameBuyAndSellToken,
    ZeroAmount,
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
            PlacementError::WrongDelegate => (
                "WrongDelegate",
                "the delegation must target the settlement state PDA",
            ),
            PlacementError::SameBuyAndSellToken => {
                ("SameBuyAndSellToken", "buy and sell token must differ")
            }
            PlacementError::ZeroAmount => ("ZeroAmount", "order amounts must not be zero"),
            PlacementError::InsufficientValidTo => (
                "InsufficientValidTo",
                "validTo lies closer than the minimum validity",
            ),
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
    let mut order = validate(sponsoring, &transaction, state.validation().min_validity)?;
    order.presigned_transaction = params.transaction;

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

    // Short-circuit replays with a cheap read before the insert. A replayed
    // transaction usually dies at the blockhash check already, and the
    // insert's unique violation stays as the race-safe backstop.
    let duplicate = db::order_exists(state.pool(), &order.uid)
        .await
        .map_err(|err| internal_error_reply(err, "order existence check failed"))?;
    if duplicate {
        return Err(PlacementError::DuplicatedOrder.into());
    }

    // The link is best-effort: a quote that is missing, expired, or not the
    // one this order came from is dropped with a warning instead of
    // rejecting an otherwise valid order.
    order.quote_id = match params.quote_id {
        Some(id) => link_quote(state.pool(), id, &order).await,
        None => None,
    };

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
    min_validity: std::time::Duration,
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
    let Some((instruction, preparations)) = message.instructions().split_last() else {
        return Err(PlacementError::InvalidTransaction(
            "the transaction carries no instructions",
        ));
    };
    if keys.get(usize::from(instruction.program_id_index)) != Some(&sponsoring.settlement_program) {
        return Err(PlacementError::InvalidTransaction(
            "the transaction must end with a CreateOrder instruction",
        ));
    }
    let accounts = resolve_accounts(instruction, keys)?;
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
    // The signature loop below skips the funder's slot, so a funder-owned
    // intent would be authorized by the countersign alone. The owner must
    // also sit among the required signers: the header is client-controlled,
    // and an owner outside it would fail only at broadcast, after winning.
    if intent.owner == sponsoring.funder {
        return Err(PlacementError::InvalidTransaction(
            "the funder cannot own a sponsored order",
        ));
    }
    let signers = usize::from(message.header().num_required_signatures);
    if !keys.iter().take(signers).any(|key| *key == intent.owner) {
        return Err(PlacementError::InvalidSignature);
    }
    if intent.sell_mint == intent.buy_mint {
        return Err(PlacementError::SameBuyAndSellToken);
    }
    if intent.sell_amount == 0 || intent.buy_amount == 0 {
        return Err(PlacementError::ZeroAmount);
    }
    let order_pda = find_order_pda(&sponsoring.settlement_program, &uid).0;
    if *input.order_pda != order_pda {
        return Err(PlacementError::WrongOrderPda);
    }
    let earliest =
        chrono::Utc::now().timestamp() + i64::try_from(min_validity.as_secs()).unwrap_or(i64::MAX);
    if i64::from(intent.valid_to) <= earliest {
        return Err(PlacementError::InsufficientValidTo);
    }

    // The preparation instructions may only follow the template: each step
    // at most once, in template order. The buy-account creation is the one
    // mandatory step, everything else is omittable.
    let state_pda = find_state_pda(&sponsoring.settlement_program).0;
    let mut last_step = 0;
    for preparation in preparations {
        let step = preparation_step(sponsoring, &state_pda, &intent, keys, preparation)?;
        if step <= last_step {
            return Err(PlacementError::InvalidTransaction(
                "the instructions do not follow the sponsored template order",
            ));
        }
        last_step = step;
    }
    if last_step != CREATE_DESTINATION {
        return Err(PlacementError::InvalidTransaction(
            "the bundle must create the buy token account",
        ));
    }

    // Every required signer except the funder must have signed: the funder's
    // slot stays a placeholder until the autopilot countersigns.
    let message_bytes = message.serialize();
    for (index, key) in keys.iter().take(signers).enumerate() {
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

    Ok(build_order(intent, uid, order_pda))
}

/// The quote id to store on the order: `id` when the stored quote matches
/// the order (same pair and side, same fixed amount, unexpired), `None`
/// otherwise.
/// TODO: once fee policies consume the link, a miss must re-quote and link
/// the fresh quote instead of dropping the link, like the EVM orderbook's
/// `find_quote` fallback, so every order carries a quote.
async fn link_quote(pool: &sqlx::PgPool, id: i64, order: &db::SponsoredOrder) -> Option<i64> {
    let quote = match db::read_quote(pool, id).await {
        Ok(Some(quote)) => quote,
        Ok(None) => {
            tracing::warn!(id, "quote link dropped, no such quote");
            return None;
        }
        Err(err) => {
            tracing::warn!(id, ?err, "quote link dropped, lookup failed");
            return None;
        }
    };
    // The unfixed side of the order carries the user's slippage, so only the
    // fixed one is expected to equal the quote's.
    let fixed_amount_matches = match order.kind {
        OrderKind::Sell => quote.sell_amount == BigDecimal::from(order.sell_amount),
        OrderKind::Buy => quote.buy_amount == BigDecimal::from(order.buy_amount),
    };
    let matches = quote.sell_token.0 == order.sell_token
        && quote.buy_token.0 == order.buy_token
        && quote.kind == order.kind
        && fixed_amount_matches
        && quote.expiration > chrono::Utc::now();
    if !matches {
        tracing::warn!(id, "quote link dropped, the quote does not match the order");
        return None;
    }
    Some(id)
}

/// Resolve an instruction's account indexes into the transaction's keys.
fn resolve_accounts(
    instruction: &CompiledInstruction,
    keys: &[Pubkey],
) -> Result<Vec<Pubkey>, PlacementError> {
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
    Ok(accounts)
}

/// Template positions of the preparation steps. Ascending ranks encode the
/// only accepted instruction order.
const WRAP_CREATE: u8 = 1;
const WRAP_TRANSFER: u8 = 2;
const WRAP_SYNC: u8 = 3;
const APPROVE: u8 = 4;
const CREATE_DESTINATION: u8 = 5;

/// Classify one preparation instruction against the sponsored template and
/// pin every account it touches to the order. The funder pays for the whole
/// transaction, so anything the template does not name is rejected.
fn preparation_step(
    sponsoring: &Sponsoring,
    state_pda: &Pubkey,
    intent: &OrderIntent,
    keys: &[Pubkey],
    instruction: &CompiledInstruction,
) -> Result<u8, PlacementError> {
    let Some(program) = keys.get(usize::from(instruction.program_id_index)) else {
        return Err(PlacementError::InvalidTransaction(
            "an account index is out of range",
        ));
    };
    let accounts = resolve_accounts(instruction, keys)?;
    // Wrap steps only make sense when the order sells native SOL through the
    // wSOL mint.
    let wrapped_sell = intent.sell_mint == spl_token_interface::native_mint::ID;

    if *program == solana_system_interface::program::ID {
        if !matches!(
            bincode::deserialize(&instruction.data),
            Ok(SystemInstruction::Transfer { .. })
        ) {
            return Err(PlacementError::InvalidTransaction(
                "only a transfer is accepted from the system program",
            ));
        }
        let [from, to] = accounts[..] else {
            return Err(PlacementError::InvalidTransaction(
                "a wrap transfer names a sender and a recipient",
            ));
        };
        if !wrapped_sell {
            return Err(PlacementError::InvalidTransaction(
                "wrap steps apply only to orders selling native SOL",
            ));
        }
        if from != intent.owner {
            return Err(PlacementError::InvalidTransaction(
                "the wrap transfer must come from the order owner",
            ));
        }
        if to != intent.sell_token_account {
            return Err(PlacementError::InvalidTransaction(
                "the wrap transfer must fund the sell token account",
            ));
        }
        Ok(WRAP_TRANSFER)
    } else if *program == spl_token_interface::ID {
        match TokenInstruction::unpack(&instruction.data) {
            Ok(TokenInstruction::SyncNative) => {
                let [account] = accounts[..] else {
                    return Err(PlacementError::InvalidTransaction(
                        "a sync names one account",
                    ));
                };
                if !wrapped_sell {
                    return Err(PlacementError::InvalidTransaction(
                        "wrap steps apply only to orders selling native SOL",
                    ));
                }
                if account != intent.sell_token_account {
                    return Err(PlacementError::InvalidTransaction(
                        "the sync must target the sell token account",
                    ));
                }
                Ok(WRAP_SYNC)
            }
            Ok(TokenInstruction::Approve { .. }) => {
                let [source, delegate, owner] = accounts[..] else {
                    return Err(PlacementError::InvalidTransaction(
                        "an approve names a source, a delegate, and an owner",
                    ));
                };
                approve_step(state_pda, intent, source, delegate, owner)
            }
            Ok(TokenInstruction::ApproveChecked { .. }) => {
                let [source, mint, delegate, owner] = accounts[..] else {
                    return Err(PlacementError::InvalidTransaction(
                        "a checked approve names a source, a mint, a delegate, and an owner",
                    ));
                };
                if mint != intent.sell_mint {
                    return Err(PlacementError::InvalidTransaction(
                        "the approve must cover the sell mint",
                    ));
                }
                approve_step(state_pda, intent, source, delegate, owner)
            }
            _ => Err(PlacementError::InvalidTransaction(
                "only approve and sync-native are accepted from the token program",
            )),
        }
    } else if *program == spl_associated_token_account_interface::program::ID {
        // The data byte selects Create ([] or [0]) or CreateIdempotent ([1]).
        if !matches!(instruction.data.as_slice(), [] | [0] | [1]) {
            return Err(PlacementError::InvalidTransaction(
                "only account creation is accepted from the associated token program",
            ));
        }
        let [payer, account, owner, mint, system, token_program] = accounts[..] else {
            return Err(PlacementError::InvalidTransaction(
                "an account creation names six accounts",
            ));
        };
        if system != solana_system_interface::program::ID
            || token_program != spl_token_interface::ID
        {
            return Err(PlacementError::InvalidTransaction(
                "the account creation must reference the system and token programs",
            ));
        }
        if payer != sponsoring.funder && payer != intent.owner {
            return Err(PlacementError::InvalidTransaction(
                "the account creation must be paid by the funder or the owner",
            ));
        }
        if owner != intent.owner {
            return Err(PlacementError::InvalidTransaction(
                "the created account must belong to the order owner",
            ));
        }
        if wrapped_sell && account == intent.sell_token_account && mint == intent.sell_mint {
            Ok(WRAP_CREATE)
        } else if account == intent.buy_token_account && mint == intent.buy_mint {
            Ok(CREATE_DESTINATION)
        } else {
            Err(PlacementError::InvalidTransaction(
                "the created account does not belong to the order",
            ))
        }
    } else {
        Err(PlacementError::InvalidTransaction(
            "an instruction targets a program outside the sponsored template",
        ))
    }
}

/// Pin a delegation to the order: the sell token account approves the
/// settlement state PDA, signed by the order owner.
fn approve_step(
    state_pda: &Pubkey,
    intent: &OrderIntent,
    source: Pubkey,
    delegate: Pubkey,
    owner: Pubkey,
) -> Result<u8, PlacementError> {
    if source != intent.sell_token_account {
        return Err(PlacementError::InvalidTransaction(
            "the approve must cover the sell token account",
        ));
    }
    if delegate != *state_pda {
        return Err(PlacementError::WrongDelegate);
    }
    if owner != intent.owner {
        return Err(PlacementError::InvalidTransaction(
            "the approve owner must be the order owner",
        ));
    }
    Ok(APPROVE)
}

/// Assemble the order row from the validated intent.
fn build_order(
    intent: OrderIntent,
    uid: solana_sdk::hash::Hash,
    order_pda: Pubkey,
) -> db::SponsoredOrder {
    db::SponsoredOrder {
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
        quote_id: None,
    }
}
