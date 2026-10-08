//! The sponsored order placement endpoint: a partially signed creation
//! transaction comes in, and every order field derives from it, so the
//! stored order and the transaction creating it on chain cannot disagree.
//!
//! The funder countersigns as fee payer, so only the whitelisted
//! preparation steps may precede the mandatory trailing `CreateOrder`: wrap
//! SOL, delegate the sell account, create the buy token account. The
//! buy-account creation is required even when the account exists (it is
//! idempotent on chain): settlement pays out to it and never creates it,
//! and the instruction proves receivability without a lookup or a race. A
//! native SOL buy has no buy token account: its payout goes to the wallet
//! and creates it when missing, so placement looks the wallet up instead.

use {
    super::mint::{ensure_settleable, is_token_program, token_mints},
    crate::infra::{
        api::{Sponsoring, State, error},
        db,
    },
    axum::{Json, http::StatusCode},
    bigdecimal::ToPrimitive,
    cow_settlement_interface::{
        SettlementError,
        data::intent::{
            Asset,
            ENCODED_NATIVE_SOL_TRANSFER,
            OrderIntent,
            OrderKind as IntentOrderKind,
            hash_bytes,
        },
        instruction::{InstructionInputParsing, create_order::CreateOrderInput},
        pda::{order::find_order_pda, state::STATE_PDA},
    },
    database::{byte_array::ByteArray, solana::OrderKind},
    serde::Deserialize,
    serde_with::{base64::Base64, serde_as},
    solana_sdk::{
        clock::MAX_PROCESSING_AGE,
        message::{VersionedMessage, compiled_instruction::CompiledInstruction},
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
    // TODO: the alias is deprecated and should be dropped in the future
    #[serde(alias = "transaction")]
    pub partially_signed_tx: Vec<u8>,
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
    InvalidNativeBuy(&'static str),
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
            PlacementError::InvalidNativeBuy(description) => ("InvalidNativeBuy", description),
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

/// Why a placement failed: a refusal the client can act on, or an internal
/// error with the step that failed.
enum Failure {
    Refused(PlacementError),
    /// A refusal the mint check shared with quoting already shaped into its
    /// reply.
    Replied(error::Reply),
    Internal {
        err: Box<dyn std::fmt::Debug + Send>,
        step: &'static str,
    },
}

impl Failure {
    fn internal(err: impl std::fmt::Debug + Send + 'static, step: &'static str) -> Self {
        Self::Internal {
            err: Box::new(err),
            step,
        }
    }
}

impl From<PlacementError> for Failure {
    fn from(error: PlacementError) -> Self {
        Self::Refused(error)
    }
}

impl From<Failure> for error::Reply {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Refused(error) => error.into(),
            Failure::Replied(reply) => reply,
            Failure::Internal { .. } => {
                error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
            }
        }
    }
}

/// Handle `POST /api/v1/orders`: validate the transaction, derive the order
/// from it, and persist both. Answers the order uid like the EVM orderbook.
pub async fn create_order(
    state: axum::extract::State<State>,
    Json(params): Json<Params>,
) -> Result<(StatusCode, Json<String>), error::Reply> {
    place(state, params).await.map_err(|failure| {
        match &failure {
            Failure::Refused(error) => tracing::debug!(err = ?error, "error creating order"),
            Failure::Replied(reply) => tracing::debug!(err = ?reply, "error creating order"),
            Failure::Internal { err, step } => tracing::error!(?err, step, "error creating order"),
        }
        failure.into()
    })
}

async fn place(
    state: axum::extract::State<State>,
    params: Params,
) -> Result<(StatusCode, Json<String>), Failure> {
    let Some(sponsoring) = state.sponsoring() else {
        return Err(PlacementError::SponsoringDisabled.into());
    };
    let transaction: VersionedTransaction = bincode::deserialize(&params.partially_signed_tx)
        .map_err(|_| {
            PlacementError::InvalidTransaction("the bytes do not decode to a transaction")
        })?;
    let (mut order, token_programs) =
        validate(sponsoring, &transaction, state.validation().min_validity)?;
    order.presigned_transaction = params.partially_signed_tx;
    check_accounts(sponsoring, &order, &token_programs).await?;

    // The countersign re-checks freshness, so the stored expiry only has to
    // be an upper bound: the tip cannot have moved past the blockhash's own
    // last valid height by more than the maximum age.
    let blockhash = transaction.message.recent_blockhash();
    let valid = sponsoring
        .rpc
        .is_blockhash_valid(blockhash)
        .await
        .map_err(|err| Failure::internal(err, "blockhash validity check"))?;
    if !valid {
        return Err(PlacementError::BlockhashExpired.into());
    }
    let height = sponsoring
        .rpc
        .block_height()
        .await
        .map_err(|err| Failure::internal(err, "block height fetch"))?;
    order.last_valid_block_height = u64::from(height) + MAX_PROCESSING_AGE as u64;

    // Short-circuit replays with a cheap read before the insert. A replayed
    // transaction usually dies at the blockhash check already, and the
    // insert's unique violation stays as the race-safe backstop.
    let duplicate = db::order_exists(state.pool(), &order.uid.0)
        .await
        .map_err(|err| Failure::internal(err, "order existence check"))?;
    if duplicate {
        return Err(PlacementError::DuplicatedOrder.into());
    }

    // The link is best-effort: a quote that is missing, expired, or not the
    // one this order came from is dropped with a warning instead of
    // rejecting an otherwise valid order.
    let quote = match params.quote_id {
        Some(id) => link_quote(state.pool(), id, &order).await,
        None => None,
    };

    let uid = const_hex::encode_prefixed(order.uid.0);
    if let Err(err) = db::insert_sponsored_order(state.pool(), &order, quote.as_ref()).await {
        let duplicate = err
            .downcast_ref::<sqlx::Error>()
            .and_then(|err| err.as_database_error())
            .is_some_and(|db| db.is_unique_violation());
        if duplicate {
            return Err(PlacementError::DuplicatedOrder.into());
        }
        return Err(Failure::internal(err, "sponsored order insert"));
    }
    tracing::info!(order_uid = %uid, "order created");
    Ok((StatusCode::CREATED, Json(uid)))
}

/// Check the transaction is exactly the sponsored-creation shape and derive
/// the order from it. The expiry and transaction bytes are filled by the
/// caller. Also returns the `(mint, token program)` pairs the preparation
/// steps name, for the caller to check against the mints on chain.
fn validate(
    sponsoring: &Sponsoring,
    transaction: &VersionedTransaction,
    min_validity: std::time::Duration,
) -> Result<(db::SponsoredOrder, Vec<(Pubkey, Pubkey)>), PlacementError> {
    let message = &transaction.message;
    // A v1 message carries its priority fee in its config, out of reach of
    // the compute-budget pricing below, and the funder would pay it.
    if matches!(message, VersionedMessage::V1(_)) {
        return Err(PlacementError::InvalidTransaction(
            "a creation transaction must be legacy or v0",
        ));
    }
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
    // Wallets wrap the bundle in instructions of their own, before and after
    // ours, so those sit outside the template.
    let mut bundle = Vec::with_capacity(message.instructions().len());
    let mut compute_budget = ComputeBudget::default();
    for instruction in message.instructions() {
        match keys.get(usize::from(instruction.program_id_index)) {
            Some(&solana_compute_budget_interface::ID) => compute_budget.read(&instruction.data)?,
            Some(&LIGHTHOUSE_PROGRAM) => check_lighthouse(&instruction.data)?,
            _ => bundle.push(instruction),
        }
    }
    // The funder is fee payer, so the priority fee the client asked for comes
    // out of its balance.
    let priority_fee = compute_budget.max_priority_fee_lamports();
    if priority_fee > u128::from(sponsoring.max_priority_fee_lamports) {
        tracing::debug!(
            price = ?compute_budget.price,
            limit = ?compute_budget.limit,
            %priority_fee,
            "priority fee above the sponsored ceiling"
        );
        return Err(PlacementError::InvalidTransaction(
            "the priority fee is above the sponsored ceiling",
        ));
    }
    let Some((instruction, preparations)) = bundle.split_last() else {
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
    let intent = OrderIntent::try_from(&input.intent_bytes).map_err(|err| {
        if err == SettlementError::ZeroOrderAmount.into() {
            PlacementError::ZeroAmount
        } else {
            PlacementError::InvalidTransaction("the intent bytes do not decode")
        }
    })?;
    let uid = hash_bytes(&input.intent_bytes);
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
    // Buying native SOL counts as buying wSOL.
    let buy_mint = match &intent.buy {
        Asset::TokenProgram(buy) => buy.mint,
        Asset::Native(_) => spl_token_interface::native_mint::ID,
    };
    if intent.sell.mint == buy_mint {
        return Err(PlacementError::SameBuyAndSellToken);
    }
    let native_buy = matches!(intent.buy, Asset::Native(_));
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
    // at most once, in template order. The buy-account creation is mandatory
    // for a token buy, everything else is omittable.
    let state_pda = STATE_PDA;
    let mut last_step = 0;
    let mut token_programs = Vec::new();
    for preparation in preparations {
        let (step, token_program) =
            preparation_step(sponsoring, &state_pda, &intent, keys, preparation)?;
        if step <= last_step {
            return Err(PlacementError::InvalidTransaction(
                "the instructions do not follow the sponsored template order",
            ));
        }
        last_step = step;
        token_programs.extend(token_program);
    }
    if !native_buy && last_step != CREATE_DESTINATION {
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

    Ok((build_order(intent, uid, order_pda), token_programs))
}

/// Reject an order on a mint the settlement program cannot move, a
/// preparation step naming a token program that does not own its mint, and a
/// native SOL buy paying an account the System Program does not own or
/// leaving its wallet under the rent-exempt minimum. Lamports paid to a
/// program or a sysvar revert the settlement, and a program-owned account
/// strands them. The payout creates a missing wallet.
async fn check_accounts(
    sponsoring: &Sponsoring,
    order: &db::SponsoredOrder,
    token_programs: &[(Pubkey, Pubkey)],
) -> Result<(), Failure> {
    let [sell, buy, buy_account] = [order.sell_token, order.buy_token, order.buy_token_account]
        .map(|key| Pubkey::new_from_array(key.0));
    let wallet = (buy == ENCODED_NATIVE_SOL_TRANSFER).then_some(buy_account);
    let mints: Vec<Pubkey> = token_mints(sell, buy).collect();
    let lookup = sponsoring.mints.lookup(mints.iter().copied());
    let accounts = sponsoring
        .rpc
        .multiple_accounts(lookup.unread().chain(wallet))
        .await
        .map_err(|err| Failure::internal(err, "order account lookup"))?;
    let verdicts = lookup.resolve(&accounts);
    ensure_settleable(&verdicts, mints).map_err(Failure::Replied)?;
    if let Some(wallet) = wallet
        && accounts
            .get(&wallet)
            .is_some_and(|account| account.owner != solana_system_interface::program::ID)
    {
        return Err(PlacementError::InvalidNativeBuy(
            "a native SOL buy must pay a wallet owned by the System Program",
        )
        .into());
    }
    if let Some(wallet) = wallet
        && !super::receivable_native_payout(
            accounts.get(&wallet),
            order.buy_amount,
            order.partially_fillable,
        )
    {
        return Err(PlacementError::InvalidNativeBuy(
            "a native SOL buy must leave its wallet rent-exempt",
        )
        .into());
    }
    // The parser matched every preparation step to the sell or buy mint, both
    // read above, so each pair has a verdict.
    let owned = token_programs.iter().all(|(mint, program)| {
        verdicts
            .get(mint)
            .is_some_and(|verdict| verdict.is_ok_and(|owner| owner.address() == *program))
    });
    if !owned {
        return Err(PlacementError::InvalidTransaction(
            "a preparation step names a token program that does not own its mint",
        )
        .into());
    }
    Ok(())
}

/// The quote copy to store under the order: filled when the stored quote
/// matches the order (same pair and side, same fixed amount, unexpired),
/// `None` otherwise. The unfixed side carries the user's slippage and stays
/// unchecked, like the EVM `find_quote` match, so the linked quote's
/// promised price is not a trustworthy value.
/// TODO: once fee policies consume the link, a miss must re-quote and link
/// the fresh quote instead of dropping the link, like the EVM orderbook's
/// `find_quote` fallback, and the match must tighten (the unfixed side
/// within the order's slippage) so the consumed price cannot be shopped in.
async fn link_quote(
    pool: &sqlx::PgPool,
    id: i64,
    order: &db::SponsoredOrder,
) -> Option<db::OrderQuote> {
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
        OrderKind::Sell => quote.sell_amount.to_u64() == Some(order.sell_amount),
        OrderKind::Buy => quote.buy_amount.to_u64() == Some(order.buy_amount),
    };
    let matches = quote.sell_token == order.sell_token
        && quote.buy_token == order.buy_token
        && quote.kind == order.kind
        && fixed_amount_matches
        && quote.expiration > chrono::Utc::now();
    if !matches {
        tracing::warn!(id, "quote link dropped, the quote does not match the order");
        return None;
    }
    Some(db::OrderQuote {
        quote_id: id,
        sell_amount: quote.sell_amount,
        buy_amount: quote.buy_amount,
        solver: quote.solver,
    })
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

/// The compute units a transaction can consume at most, whatever it declares.
/// A transaction that names no limit is priced against this, since the units
/// the runtime would otherwise grant depend on which programs each
/// instruction calls.
const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// Lighthouse, the guard program wallets wrap a transaction in to assert the
/// state it leaves behind.
const LIGHTHOUSE_PROGRAM: Pubkey =
    Pubkey::from_str_const("L2TExMFKdjpN9kozasaurPirfHy9P8sbXoAN1qA3S95");

/// Lighthouse's assertion instructions, `AssertAccountData` through
/// `AssertBubblegumTreeConfigAccount`. Each reads account state and aborts the
/// transaction on a mismatch, so none of them spends the funder's lamports.
/// The two variants below the range, `MemoryWrite` and `MemoryClose`, name a
/// payer that funds a memory account's rent, and on a sponsored creation that
/// payer can be the funder.
const LIGHTHOUSE_ASSERTIONS: std::ops::RangeInclusive<u8> = 2..=17;

/// Accept only a Lighthouse assertion. The program is upgradeable at a fixed
/// address, so an instruction this build does not know is refused rather than
/// assumed harmless.
fn check_lighthouse(data: &[u8]) -> Result<(), PlacementError> {
    match data.first() {
        Some(discriminator) if LIGHTHOUSE_ASSERTIONS.contains(discriminator) => Ok(()),
        _ => Err(PlacementError::InvalidTransaction(
            "only lighthouse assertions are accepted on a sponsored creation",
        )),
    }
}

/// What the client asked the runtime to charge for priority.
#[derive(Default)]
struct ComputeBudget {
    /// Micro-lamports per compute unit.
    price: Option<u64>,
    /// Compute units the transaction may consume.
    limit: Option<u32>,
}

impl ComputeBudget {
    /// Read one compute-budget instruction. Duplicates of a kind are rejected
    /// because the runtime rejects them too, so accepting one would price the
    /// transaction off a value that never takes effect.
    fn read(&mut self, data: &[u8]) -> Result<(), PlacementError> {
        // Discriminators of `SetComputeUnitLimit` (u32) and
        // `SetComputeUnitPrice` (u64), both little-endian.
        let (slot_taken, malformed) = match data.split_first() {
            Some((2, limit)) => {
                let limit = limit
                    .try_into()
                    .map(u32::from_le_bytes)
                    .map_err(|_| "malformed compute unit limit");
                (self.limit.is_some(), limit.map(|v| self.limit = Some(v)))
            }
            Some((3, price)) => {
                let price = price
                    .try_into()
                    .map(u64::from_le_bytes)
                    .map_err(|_| "malformed compute unit price");
                (self.price.is_some(), price.map(|v| self.price = Some(v)))
            }
            // Heap frames and data size limits cost the funder nothing.
            _ => return Ok(()),
        };
        if slot_taken {
            return Err(PlacementError::InvalidTransaction(
                "the transaction sets a compute budget twice",
            ));
        }
        malformed.map_err(PlacementError::InvalidTransaction)
    }

    /// The most the transaction could pay in priority fee, in lamports,
    /// rounded up. An undeclared limit is priced at the network ceiling.
    fn max_priority_fee_lamports(&self) -> u128 {
        let Some(price) = self.price else {
            return 0;
        };
        let limit = self.limit.unwrap_or(MAX_COMPUTE_UNIT_LIMIT);
        // Micro-lamports per unit times units, rounded up to whole lamports.
        (u128::from(price) * u128::from(limit)).div_ceil(1_000_000)
    }
}

/// Classify one preparation instruction against the sponsored template and
/// pin every account it touches to the order. The funder pays for the whole
/// transaction, so anything the template does not name is rejected. Returns
/// the step's template position and, for a token step, its mint with the
/// token program it names.
fn preparation_step(
    sponsoring: &Sponsoring,
    state_pda: &Pubkey,
    intent: &OrderIntent,
    keys: &[Pubkey],
    instruction: &CompiledInstruction,
) -> Result<(u8, Option<(Pubkey, Pubkey)>), PlacementError> {
    let Some(program) = keys.get(usize::from(instruction.program_id_index)) else {
        return Err(PlacementError::InvalidTransaction(
            "an account index is out of range",
        ));
    };
    let accounts = resolve_accounts(instruction, keys)?;
    // Wrap steps only make sense when the order sells native SOL through the
    // wSOL mint.
    let wrapped_sell = intent.sell.mint == spl_token_interface::native_mint::ID;

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
        if to != intent.sell.token_account {
            return Err(PlacementError::InvalidTransaction(
                "the wrap transfer must fund the sell token account",
            ));
        }
        Ok((WRAP_TRANSFER, None))
    } else if is_token_program(program) {
        // Token-2022 encodes these instructions the way SPL Token does.
        let step = match TokenInstruction::unpack(&instruction.data) {
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
                if account != intent.sell.token_account {
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
                if mint != intent.sell.mint {
                    return Err(PlacementError::InvalidTransaction(
                        "the approve must cover the sell mint",
                    ));
                }
                approve_step(state_pda, intent, source, delegate, owner)
            }
            _ => Err(PlacementError::InvalidTransaction(
                "only approve and sync-native are accepted from the token program",
            )),
        }?;
        Ok((step, Some((intent.sell.mint, *program))))
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
        if system != solana_system_interface::program::ID || !is_token_program(&token_program) {
            return Err(PlacementError::InvalidTransaction(
                "the account creation must reference the system and token programs",
            ));
        }
        if payer != sponsoring.funder && payer != intent.owner {
            return Err(PlacementError::InvalidTransaction(
                "the account creation must be paid by the funder or the owner",
            ));
        }
        if wrapped_sell && account == intent.sell.token_account && mint == intent.sell.mint {
            // An order sells its owner's funds, so the wSOL account is theirs.
            if owner != intent.owner {
                return Err(PlacementError::InvalidTransaction(
                    "the created sell token account must belong to the order owner",
                ));
            }
            Ok((WRAP_CREATE, Some((mint, token_program))))
        } else if let Asset::TokenProgram(buy) = &intent.buy
            && account == buy.token_account
            && mint == buy.mint
        {
            // Any wallet may receive the proceeds: settlement pays out to the
            // account the intent names, whoever owns it.
            Ok((CREATE_DESTINATION, Some((mint, token_program))))
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
    if source != intent.sell.token_account {
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
    let (buy_mint, buy_token_account) = intent.buy.encode();
    db::SponsoredOrder {
        uid: ByteArray(uid.to_bytes()),
        owner: ByteArray(intent.owner.to_bytes()),
        sell_token: ByteArray(intent.sell.mint.to_bytes()),
        buy_token: ByteArray(buy_mint.to_bytes()),
        sell_token_account: ByteArray(intent.sell.token_account.to_bytes()),
        buy_token_account: ByteArray(buy_token_account.to_bytes()),
        sell_amount: intent.sell_amount.get(),
        buy_amount: intent.buy_amount.get(),
        valid_to: intent.valid_to,
        kind: match intent.flags.kind {
            IntentOrderKind::Sell => OrderKind::Sell,
            IntentOrderKind::Buy => OrderKind::Buy,
        },
        partially_fillable: intent.flags.partially_fillable,
        app_data: ByteArray(intent.app_data),
        order_pda: ByteArray(order_pda.to_bytes()),
        presigned_transaction: Vec::new(),
        last_valid_block_height: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_limit(units: u32) -> Vec<u8> {
        std::iter::once(2u8)
            .chain(units.to_le_bytes())
            .collect::<Vec<_>>()
    }

    fn set_price(micro_lamports: u64) -> Vec<u8> {
        std::iter::once(3u8)
            .chain(micro_lamports.to_le_bytes())
            .collect::<Vec<_>>()
    }

    /// The fee is the product, so a steep price over few units stays cheap
    /// while a modest price over the whole compute ceiling does not.
    #[test]
    fn the_fee_is_the_price_times_the_limit() {
        let mut steep = ComputeBudget::default();
        steep.read(&set_price(1_000_000)).unwrap();
        steep.read(&set_limit(20_000)).unwrap();
        assert_eq!(steep.max_priority_fee_lamports(), 20_000);

        let mut wide = ComputeBudget::default();
        wide.read(&set_price(1_000)).unwrap();
        wide.read(&set_limit(MAX_COMPUTE_UNIT_LIMIT)).unwrap();
        assert_eq!(wide.max_priority_fee_lamports(), 1_400);
    }

    /// No price means no priority fee. A price without a limit is priced at
    /// the ceiling, since the transaction may consume up to it.
    #[test]
    fn an_undeclared_limit_is_priced_at_the_ceiling() {
        assert_eq!(ComputeBudget::default().max_priority_fee_lamports(), 0);

        let mut priced = ComputeBudget::default();
        priced.read(&set_price(1_000_000)).unwrap();
        assert_eq!(priced.max_priority_fee_lamports(), 1_400_000);
    }

    /// The runtime rejects a repeated compute-budget instruction, so pricing
    /// the transaction off the first one would read a value that never runs.
    #[test]
    fn a_repeated_compute_budget_is_rejected() {
        let mut budget = ComputeBudget::default();
        budget.read(&set_price(10)).unwrap();
        assert!(budget.read(&set_price(20)).is_err());

        let mut budget = ComputeBudget::default();
        budget.read(&set_limit(10)).unwrap();
        assert!(budget.read(&set_limit(20)).is_err());
    }

    /// A truncated payload is refused rather than read as a smaller number.
    #[test]
    fn a_malformed_payload_is_rejected() {
        let mut budget = ComputeBudget::default();
        assert!(budget.read(&[3, 1, 2, 3]).is_err());
        assert!(budget.read(&[2, 1]).is_err());
        // Variants that cost the funder nothing are ignored, not parsed.
        assert!(budget.read(&[1, 0, 0, 4, 0]).is_ok());
    }

    #[test]
    fn transaction_is_an_alias_of_partially_signed_tx() {
        let params: Params =
            serde_json::from_value(serde_json::json!({ "transaction": "AQID" })).unwrap();
        assert_eq!(params.partially_signed_tx, [1, 2, 3]);
    }
}
