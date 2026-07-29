#![expect(dead_code)]
//! The decoder pulls `StreamUpdate`s from the ingester, decodes
//! settlement-program and SolFlow transactions, and persists typed events.

// TODO: `decode_solflow` and the persist path are stubbed. `run` drains the
// channel, decodes settlement instructions into `SettlementEvent`s, and drops
// them until the persistence adapter lands.

use {
    crate::{
        persistence::Persistence,
        types::{
            Signature,
            channel::StreamUpdate,
            errors::{DecodeError, PersistenceError},
            events::{SettlementEvent, TradeDelta},
            order::OrderUid,
            slot::Slot,
            tx::{ResolvedInstruction, TxContext},
            wire::SubscribeUpdateTransactionInfo,
        },
    },
    bytes::Bytes,
    settlement_interface::{
        Pubkey as InterfacePubkey,
        SettlementInstruction,
        data::intent::EncodedOrderIntent,
        instruction::settle::recover_counterpart,
        recover_discriminator,
    },
    solana_sdk::pubkey::Pubkey,
    tokio::sync::mpsc::Receiver,
};

/// Decoder component.
pub(crate) struct Decoder {
    /// Persistence layer.
    pub persistence: Persistence,

    /// Incoming `StreamUpdate` from the ingester.
    pub rx: Receiver<StreamUpdate>,

    /// Settlement program id (filter target for the decoder).
    pub settlement_program: Pubkey,

    /// SolFlow program id (filter target for the decoder).
    pub solflow_program: Pubkey,
}

impl Decoder {
    /// Construct a new decoder. The caller owns the channel capacity decision.
    pub fn new(
        persistence: Persistence,
        rx: Receiver<StreamUpdate>,
        settlement_program: Pubkey,
        solflow_program: Pubkey,
    ) -> Self {
        Self {
            persistence,
            rx,
            settlement_program,
            solflow_program,
        }
    }

    /// Main loop. Drains the channel and decodes each transaction's tracked
    /// instructions. Returns when the ingester drops the sender.
    pub async fn run(&mut self) -> Result<(), PersistenceError> {
        while let Some(update) = self.rx.recv().await {
            let StreamUpdate::Tx {
                slot,
                signature,
                inner,
            } = update;
            self.decode(&inner, slot, signature);
        }
        Ok(())
    }

    /// Decode one transaction's tracked instructions into domain events. The
    /// settlement half runs through the pure [`decode_settlement`]; the SolFlow
    /// half is still stubbed.
    fn decode(&self, tx: &SubscribeUpdateTransactionInfo, slot: Slot, signature: Signature) {
        let instructions =
            relevant_instructions(tx, &self.settlement_program, &self.solflow_program);
        if instructions.is_empty() {
            return;
        }

        // `relevant_instructions` reconstructs the account list internally to
        // resolve program ids; rebuild it once here so the decode can resolve
        // account indices to pubkeys too.
        let account_keys = build_account_keys(tx);
        let post_token_balances = tx
            .meta
            .as_ref()
            .map(|meta| meta.post_token_balances.clone())
            .unwrap_or_default();
        let ctx = TxContext {
            slot,
            signature,
            account_keys,
            post_token_balances,
        };

        // `relevant_instructions` yields only settlement and SolFlow instructions.
        // The settlement set is decoded here and the SolFlow set below, so the two
        // filters are exhaustive and nothing is silently dropped.
        let settlement: Vec<ResolvedInstruction> = instructions
            .iter()
            .filter(|instruction| instruction.program_id == self.settlement_program)
            .cloned()
            .collect();

        // TODO: resolve order PDAs against persisted order rows once the store
        // adapter lands. Until then nothing resolves, so `SettlementFinalized`
        // events carry the tx-level fields with empty trades.
        // TODO: skip transactions whose `meta.err` is set: a reverted settlement
        // or order creation must not emit an event. Deferred until the persist
        // path is wired (nothing is persisted yet).
        let events = decode_settlement(&settlement, &ctx, |_order_pda| None);

        for instruction in instructions
            .iter()
            .filter(|instruction| instruction.program_id == self.solflow_program)
        {
            self.decode_solflow(instruction);
        }

        // TODO: persist `events` once the persistence adapter lands; for now the
        // decode runs end to end but its output is dropped.
        tracing::debug!(
            slot = %ctx.slot,
            event_count = events.len(),
            "decoded settlement events"
        );
    }

    /// TODO: decode the SolFlow instruction data. The on-chain program does not
    /// exist yet.
    fn decode_solflow(&self, instruction: &ResolvedInstruction) {
        tracing::debug!(
            instruction_index = instruction.instruction_index,
            "sol_flow instruction decode not implemented"
        );
    }
}

/// Auction id is not carried on the `BeginSettle` wire yet, so every
/// `SettlementFinalized` uses this fixed placeholder.
// TODO: not in the BeginSettle wire yet; use the real value once the program
// emits it.
const PLACEHOLDER_AUCTION_ID: u64 = 0;

/// The buy-side push amount is not carried on the `FinalizeSettle` wire yet, so
/// every trade's `amount_received_delta` uses this fixed placeholder.
// TODO: FinalizeSettle carries no push amount yet; use the real value once it
// does.
const PLACEHOLDER_AMOUNT_RECEIVED: u64 = 0;

/// Position of the `created_by` account in a `CreateOrder`'s account list
/// `[owner (S), created_by (W,S), order_pda (W), system_program (R)]`.
const CREATE_ORDER_CREATED_BY: usize = 1;

/// Accounts a `BeginSettle` carries before its per-order accounts:
/// `[instructions_sysvar, state_pda, token_program]`.
const BEGIN_SETTLE_FIXED_ACCOUNTS: usize = 3;

/// Accounts a `CreateBuffer` carries before its per-buffer `(buffer_pda, mint)`
/// pairs: `[payer, system_program, token_program]`.
const CREATE_BUFFER_SHARED_ACCOUNTS: usize = 3;

/// Order fields the `BeginSettle` wire does not carry, looked up per order PDA
/// through an injected resolver so the decode stays a pure function. A future
/// PR backs the resolver with the persisted order rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedOrder {
    /// UID of the order held at the PDA.
    order_uid: OrderUid,
    /// Whether the order is fully filled after this settlement.
    order_fulfilled: bool,
}

/// Decode the settlement-program instructions of one transaction into domain
/// events.
///
/// Pure: every tx-level input arrives through `ctx`, and any order field the
/// `BeginSettle` wire does not carry is resolved through `resolve_order` (keyed
/// on the order PDA), so tests can inject a canned map. `instructions` must be
/// the transaction's settlement-program instructions, in execution order.
fn decode_settlement(
    instructions: &[ResolvedInstruction],
    ctx: &TxContext,
    resolve_order: impl Fn(&Pubkey) -> Option<ResolvedOrder>,
) -> Vec<SettlementEvent> {
    let mut events = Vec::new();

    // Instructions decodable on their own, without tx-level pairing.
    for instruction in instructions {
        let Ok((discriminator, body)) = recover_discriminator(&instruction.data) else {
            tracing::debug!(
                instruction_index = instruction.instruction_index,
                "settlement instruction with an unknown discriminator, skipping"
            );
            continue;
        };
        // A landed (non-reverted) transaction carries valid instruction data, so
        // a decode failure here means a decoder bug or an unannounced program
        // layout change, not a normal case. Surface it as a warning rather than
        // dropping it silently. Once the persistence adapter lands these route to
        // the dead-letter table.
        let decoded = match discriminator {
            SettlementInstruction::CreateOrder => {
                decode_order_created(instruction, body, &ctx.account_keys).map(|event| vec![event])
            }
            SettlementInstruction::CreateBuffer => {
                decode_buffers_created(instruction, &ctx.account_keys)
            }
            // Bootstrap only, no domain event.
            SettlementInstruction::Initialize => Ok(Vec::new()),
            // Paired below, once both halves of the settlement are in hand.
            SettlementInstruction::BeginSettle | SettlementInstruction::FinalizeSettle => {
                Ok(Vec::new())
            }
        };
        match decoded {
            Ok(decoded_events) => events.extend(decoded_events),
            Err(err) => tracing::warn!(
                instruction_index = instruction.instruction_index,
                %err,
                "failed to decode settlement instruction"
            ),
        }
    }

    // A `BeginSettle` plus the `FinalizeSettle` it names make one
    // `SettlementFinalized`; pairing needs the whole transaction.
    events.extend(decode_settlements_finalized(
        instructions,
        ctx,
        &resolve_order,
    ));
    events
}

/// `CreateOrder` -> `OrderCreated`. The instruction data is the encoded order
/// intent: its hash is the order UID and it carries the owner. `created_by` is
/// resolved from the instruction's account list.
fn decode_order_created(
    instruction: &ResolvedInstruction,
    body: &[u8],
    account_keys: &[Pubkey],
) -> Result<SettlementEvent, DecodeError> {
    let intent_bytes: &[u8; EncodedOrderIntent::SIZE] =
        body.try_into().map_err(|_| DecodeError::SchemaMismatch)?;
    let (intent, uid) = EncodedOrderIntent::decode_and_hash(intent_bytes)
        .map_err(|_| DecodeError::SchemaMismatch)?;
    let created_by = resolve_account(instruction, account_keys, CREATE_ORDER_CREATED_BY)
        .ok_or(DecodeError::SchemaMismatch)?;
    Ok(SettlementEvent::OrderCreated {
        order_uid: OrderUid(uid.to_bytes()),
        owner: to_sdk_pubkey(intent.owner),
        created_by,
    })
}

/// `CreateBuffer` -> one `BufferCreated` per created buffer. The wire body is
/// empty; each buffer is a trailing `(buffer_pda, mint)` account pair after the
/// shared accounts, and the event's token is the buffer's mint.
fn decode_buffers_created(
    instruction: &ResolvedInstruction,
    account_keys: &[Pubkey],
) -> Result<Vec<SettlementEvent>, DecodeError> {
    let per_buffer = instruction
        .accounts
        .get(CREATE_BUFFER_SHARED_ACCOUNTS..)
        .ok_or(DecodeError::SchemaMismatch)?;
    let (pairs, remainder) = per_buffer.as_chunks::<2>();
    if !remainder.is_empty() || pairs.is_empty() {
        return Err(DecodeError::SchemaMismatch);
    }
    pairs
        .iter()
        .map(|pair| {
            // Each pair is `[buffer_pda_index, mint_index]`; the event token is
            // the buffer's mint.
            let token = account_keys
                .get(usize::from(pair[1]))
                .ok_or(DecodeError::SchemaMismatch)?;
            Ok(SettlementEvent::BufferCreated { token: *token })
        })
        .collect()
}

/// Pair each `BeginSettle` with the `FinalizeSettle` it names and emit one
/// `SettlementFinalized` per pair.
///
/// Pairing is by index: a `BeginSettle` body carries the top-level instruction
/// index of its `FinalizeSettle` (recovered via [`recover_counterpart`]), which
/// must match a `FinalizeSettle` present in the same transaction. It is
/// independent of the two instructions' relative order.
fn decode_settlements_finalized(
    instructions: &[ResolvedInstruction],
    ctx: &TxContext,
    resolve_order: &impl Fn(&Pubkey) -> Option<ResolvedOrder>,
) -> Vec<SettlementEvent> {
    // The solver is the transaction fee payer: the first account key, which
    // Solana guarantees is the signer that submitted the transaction.
    let Some(&solver) = ctx.account_keys.first() else {
        return Vec::new();
    };

    let mut events = Vec::new();
    for begin in instructions {
        let Ok((SettlementInstruction::BeginSettle, body)) = recover_discriminator(&begin.data)
        else {
            continue;
        };
        // Body: `[finalize_ix_index: u16 BE][n][bump×n][count×n][amount×T]`.
        // Peel the counterpart index, leaving the per-order pull layout.
        let Ok((finalize_ix_index, pull_body)) = recover_counterpart(body) else {
            continue;
        };
        // The named `FinalizeSettle` must actually be present in this tx.
        let paired = instructions.iter().any(|instruction| {
            instruction.instruction_index == u32::from(finalize_ix_index)
                && matches!(
                    recover_discriminator(&instruction.data),
                    Ok((SettlementInstruction::FinalizeSettle, _))
                )
        });
        if !paired {
            tracing::debug!(
                instruction_index = begin.instruction_index,
                finalize_ix_index,
                "BeginSettle without a paired FinalizeSettle in the tx, skipping"
            );
            continue;
        }

        let Some(orders) = parse_begin_settle_orders(pull_body) else {
            tracing::warn!(
                instruction_index = begin.instruction_index,
                "BeginSettle body did not match the expected pull layout, skipping"
            );
            continue;
        };

        let trades = orders
            .into_iter()
            .filter_map(|order| {
                let order_pda =
                    resolve_account(begin, &ctx.account_keys, order.order_pda_position)?;
                let resolved = resolve_order(&order_pda)?;
                Some(TradeDelta {
                    order_uid: resolved.order_uid,
                    amount_withdrawn_delta: order.amount_withdrawn_delta,
                    amount_received_delta: PLACEHOLDER_AMOUNT_RECEIVED,
                    order_fulfilled: resolved.order_fulfilled,
                })
            })
            .collect();

        events.push(SettlementEvent::SettlementFinalized {
            auction_id: PLACEHOLDER_AUCTION_ID,
            solver,
            tx_signature: ctx.signature,
            slot: ctx.slot,
            trades,
        });
    }
    events
}

/// One settled order recovered from a `BeginSettle` body: where its order PDA
/// sits in the instruction's account list, and the sell amount pulled for it.
struct BeginSettleOrder {
    /// Position of the order's PDA within the instruction's account list.
    order_pda_position: usize,
    /// Sum of the order's pull amounts: the sell-side `amount_withdrawn` delta.
    amount_withdrawn_delta: u64,
}

/// Hand-parse a `BeginSettle` body (after the counterpart index) into per-order
/// pull totals and the account position of each order's PDA.
//
// TODO: this duplicates the `BeginSettle` wire layout owned by
// `settlement_interface` (`[n][bump×n][count×n][amount: u64 BE ×T]`); the
// interface's own parser is account-coupled, so there is no data-only helper to
// call yet. Replace this once one exists. The layout also shifts when the
// program adds `auction_id` to the wire, so this must move in lockstep.
fn parse_begin_settle_orders(body: &[u8]) -> Option<Vec<BeginSettleOrder>> {
    let (&order_count, rest) = body.split_first()?;
    let order_count = usize::from(order_count);
    // Bumps are on-chain PDA-derivation input the indexer does not need.
    let (_bumps, rest) = rest.split_at_checked(order_count)?;
    let (counts, amount_bytes) = rest.split_at_checked(order_count)?;
    let (amounts, remainder) = amount_bytes.as_chunks::<8>();
    if !remainder.is_empty() {
        return None;
    }
    // The per-order transfer counts must sum to the number of amounts, so every
    // destination pairs with exactly one amount.
    let counts_sum = counts
        .iter()
        .map(|&count| usize::from(count))
        .sum::<usize>();
    if counts_sum != amounts.len() {
        return None;
    }

    let mut orders = Vec::with_capacity(order_count);
    let mut account_position = BEGIN_SETTLE_FIXED_ACCOUNTS;
    let mut amount_index = 0usize;
    for &count in counts {
        let count = usize::from(count);
        let mut amount_withdrawn_delta = 0u64;
        for amount in &amounts[amount_index..amount_index + count] {
            // On-chain amounts are already u64, so a sum from a single sell
            // account cannot exceed u64. Guard anyway: the stream is untrusted.
            amount_withdrawn_delta =
                amount_withdrawn_delta.checked_add(u64::from_be_bytes(*amount))?;
        }
        amount_index += count;
        orders.push(BeginSettleOrder {
            order_pda_position: account_position,
            amount_withdrawn_delta,
        });
        // Each order occupies its order PDA, its sell token account, and one
        // destination per transfer.
        account_position += 2 + count;
    }
    Some(orders)
}

/// Resolve the account at `position` in the instruction's account list to its
/// pubkey, returning `None` if the position or the resolved index is out of
/// range.
fn resolve_account(
    instruction: &ResolvedInstruction,
    account_keys: &[Pubkey],
    position: usize,
) -> Option<Pubkey> {
    let index = *instruction.accounts.get(position)?;
    account_keys.get(usize::from(index)).copied()
}

/// Bridge a `settlement_interface` pubkey to the indexer's `solana_sdk` pubkey.
/// The two crates pin different `solana-pubkey` majors, so the types differ and
/// a byte round-trip is the conversion.
fn to_sdk_pubkey(pubkey: InterfacePubkey) -> Pubkey {
    Pubkey::new_from_array(pubkey.to_bytes())
}

/// The transaction's full account list that instruction indices resolve
/// against: static keys, then ALT-loaded writable addresses, then readonly, in
/// that order. A wrong-length key becomes the zero pubkey to keep the indices
/// aligned, so it does not match a tracked program.
fn build_account_keys(tx: &SubscribeUpdateTransactionInfo) -> Vec<Pubkey> {
    let static_keys = tx
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.message.as_ref())
        .map(|message| message.account_keys.as_slice())
        .unwrap_or_default();
    let (writable, readonly) = tx
        .meta
        .as_ref()
        .map(|meta| {
            (
                meta.loaded_writable_addresses.as_slice(),
                meta.loaded_readonly_addresses.as_slice(),
            )
        })
        .unwrap_or_default();
    static_keys
        .iter()
        .chain(writable)
        .chain(readonly)
        .map(|key| Pubkey::try_from(key.as_slice()).unwrap_or_default())
        .collect()
}

/// A top-level or inner (CPI) instruction with its position, before its program
/// and account indices are resolved to pubkeys.
struct RawInstruction<'a> {
    /// Top-level instruction index. For a CPI, the top-level instruction it
    /// runs under.
    instruction_index: u32,
    /// Path within the top-level instruction's CPI tree (see
    /// `ResolvedInstruction::inner_ix_path`). Empty for a top-level
    /// instruction.
    inner_ix_path: Vec<u8>,
    /// Index of the invoked program in the account list.
    program_id_index: u32,
    /// Account-list indices of the accounts the instruction touches.
    account_indices: &'a [u8],
    /// Raw instruction data.
    data: &'a [u8],
}

impl RawInstruction<'_> {
    /// Resolve the program against `account_keys`, keeping the instruction only
    /// if that program is the settlement or SolFlow program. The `accounts`
    /// field keeps the raw account-list indices, resolving them to pubkeys is
    /// left to the decode step. Returns `None` if the program is untracked or
    /// its index is out of range.
    fn resolve_protocol_instruction(
        self,
        account_keys: &[Pubkey],
        settlement_program: &Pubkey,
        solflow_program: &Pubkey,
    ) -> Option<ResolvedInstruction> {
        let program_id = *account_keys.get(self.program_id_index as usize)?;
        if program_id != *settlement_program && program_id != *solflow_program {
            return None;
        }
        Some(ResolvedInstruction {
            program_id,
            data: Bytes::copy_from_slice(self.data),
            accounts: self.account_indices.to_vec(),
            instruction_index: self.instruction_index,
            inner_ix_path: self.inner_ix_path,
        })
    }
}

/// Solana caps the instruction stack at height 5 (top-level = height 1), so a
/// CPI path is at most 4 deep. Clamping to it guards against a corrupt
/// `stack_height` forcing a huge allocation.
const MAX_CPI_DEPTH: usize = 4;

/// Resolve every instruction against `account_keys` and keep only those whose
/// program is the settlement or SolFlow program, where settlement is often
/// reached only as a CPI. A top-level instruction and its inner instructions
/// are filtered independently, so a top-level call to an untracked program is
/// dropped while a settlement CPI nested under it is still kept. Instructions
/// are returned in on-chain execution order: each top-level instruction is
/// followed by the inner (CPI) instructions it triggered.
fn relevant_instructions(
    tx: &SubscribeUpdateTransactionInfo,
    settlement_program: &Pubkey,
    solflow_program: &Pubkey,
) -> Vec<ResolvedInstruction> {
    let account_keys = build_account_keys(tx);
    let top_level = tx
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.message.as_ref())
        .map(|message| message.instructions.as_slice())
        .unwrap_or_default();
    let inner_groups = tx
        .meta
        .as_ref()
        .map(|meta| meta.inner_instructions.as_slice())
        .unwrap_or_default();

    let mut resolved = Vec::new();
    for (index, ix) in top_level.iter().enumerate() {
        let instruction_index = index as u32;
        let top = RawInstruction {
            instruction_index,
            inner_ix_path: Vec::new(),
            program_id_index: ix.program_id_index,
            account_indices: &ix.accounts,
            data: &ix.data,
        };
        if let Some(resolved_ix) =
            top.resolve_protocol_instruction(&account_keys, settlement_program, solflow_program)
        {
            resolved.push(resolved_ix);
        }

        let Some(group) = inner_groups
            .iter()
            .find(|group| group.index == instruction_index)
        else {
            continue;
        };

        // `group.instructions` is a depth-first, execution-ordered flat list of
        // every CPI under this top-level instruction, across all nesting levels.
        // `stack_height` is the only per-CPI depth signal (2 = a direct CPI, 3 =
        // a CPI that one made, ...), so rebuild the sibling position at each
        // level from it. A dropped (untracked) inner still advances the counter,
        // so kept siblings keep their true position. A missing stack_height
        // (pre-Solana-1.14.6 data) falls back to depth 1.
        let mut path: Vec<u8> = Vec::new();
        for inner in &group.instructions {
            let depth = inner
                .stack_height
                .map(|height| height.saturating_sub(1) as usize)
                .unwrap_or(1)
                .clamp(1, MAX_CPI_DEPTH);
            if depth > path.len() {
                path.resize(depth, 0);
            } else {
                path.truncate(depth);
                if let Some(last) = path.last_mut() {
                    *last = last.saturating_add(1);
                }
            }
            let raw = RawInstruction {
                instruction_index,
                inner_ix_path: path.clone(),
                program_id_index: inner.program_id_index,
                account_indices: &inner.accounts,
                data: &inner.data,
            };
            if let Some(resolved_ix) =
                raw.resolve_protocol_instruction(&account_keys, settlement_program, solflow_program)
            {
                resolved.push(resolved_ix);
            }
        }
    }
    resolved
}

#[cfg(test)]
mod tests;
