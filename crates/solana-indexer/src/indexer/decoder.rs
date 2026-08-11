#![allow(dead_code, reason = "dead in the lib build, exercised by tests")]
//! The decoder pulls `StreamUpdate`s from the ingester, decodes
//! settlement-program and SolFlow transactions, and persists typed events.

// TODO: `decode_solflow` is a stub (the on-chain SolFlow program does not
// exist), and the `Persistence` write bodies are no-ops (no Postgres adapter).

use {
    crate::{
        persistence::Persistence,
        types::{
            Signature,
            channel::StreamUpdate,
            errors::{DecodeError, PersistenceError},
            events::{DecodedEvent, SettlementEvent, TradeDelta},
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
        instruction::{
            InstructionInputParsing,
            create_buffer::CreateBufferInput,
            create_order::CreateOrderInput,
            settle::{BeginSettleInput, FinalizeSettleInput},
        },
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

    /// Main loop. Drains the channel, decodes each transaction's tracked
    /// instructions, and hands the results to the persistence seam. Returns
    /// when the ingester drops the sender.
    ///
    /// Events and dead letters are buffered per slot and flushed once a
    /// transaction of a later slot arrives: stream resume is slot-granular
    /// (`from_slot = watermark + 1`), so the watermark may only name slots
    /// whose transactions have all been delivered. Buffering also makes it
    /// one persistence batch per slot instead of one per transaction.
    pub async fn run(&mut self) -> Result<(), PersistenceError> {
        let mut pending: Option<SlotBuffer> = None;
        while let Some(update) = self.rx.recv().await {
            let (slot, signature, inner) = match update {
                StreamUpdate::Tx {
                    slot,
                    signature,
                    inner,
                } => (slot, signature, inner),
                // A status message for a later slot proves the pending slot is
                // fully delivered. Settlement transactions can be minutes
                // apart, this keeps the flush latency at one slot instead.
                StreamUpdate::Slot { slot } => {
                    if let Some(buffer) = pending.take_if(|buffer| slot > buffer.slot) {
                        self.flush(buffer, true).await?;
                    }
                    continue;
                }
            };
            // A transaction of a later slot proves the pending slot is fully
            // delivered, so it is safe to flush and mark done.
            if let Some(buffer) = pending.take_if(|buffer| slot > buffer.slot) {
                self.flush(buffer, true).await?;
            }

            let (events, decode_failed) = match self.decode(*inner, slot, signature) {
                Ok(events) => (events, false),
                Err(PartialDecode { events }) => (events, true),
            };
            tracing::debug!(slot = %slot, event_count = events.len(), "decoded events");
            let buffer = pending.get_or_insert_with(|| SlotBuffer::new(slot));
            if slot < buffer.slot {
                // The buffered slot's watermark would claim this transaction's
                // slot as done. Its events still persist with the batch, so
                // nothing is lost, but the delivery-order assumption broke.
                tracing::warn!(
                    %slot,
                    buffered_slot = %buffer.slot,
                    "transaction arrived below the buffered slot"
                );
            }
            buffer.events.extend(events);
            if decode_failed {
                // Partial decode is expected: events that did decode persist
                // and recovery replays the whole transaction by signature,
                // idempotent writes absorb the overlap.
                buffer.dead_letters.push((signature, slot));
            }
        }
        // The stream ended, so the buffered slot may be missing transactions.
        // Keep the watermark at the previous slot: a restart redelivers the
        // whole slot and idempotent writes absorb the overlap.
        if let Some(buffer) = pending {
            self.flush(buffer, false).await?;
        }
        Ok(())
    }

    /// Persist one slot's batch. Dead letters go first: once the watermark
    /// passes a slot it is never replayed wholesale, so its replay markers
    /// must already be durable. Events and the watermark advance ride one
    /// call so the adapter can apply both in one SQL transaction. A slot cut
    /// off by the stream end (`complete = false`) keeps the watermark behind
    /// itself.
    async fn flush(&self, buffer: SlotBuffer, complete: bool) -> Result<(), PersistenceError> {
        for (signature, slot) in buffer.dead_letters {
            self.persistence.write_dead_letter(signature, slot).await?;
        }
        let watermark = if complete {
            buffer.slot
        } else {
            Slot(u64::from(buffer.slot).saturating_sub(1))
        };
        if buffer.events.is_empty() {
            if complete {
                self.persistence.write_watermark(watermark).await?;
            }
        } else {
            self.persistence
                .persist_events(buffer.events, watermark)
                .await?;
        }
        Ok(())
    }

    /// Decode one transaction's tracked instructions into domain events.
    /// `Err` means at least one settlement instruction failed to decode and
    /// carries the events that did decode. The settlement half runs through
    /// the pure [`decode_settlement`], the SolFlow half is a stub.
    #[tracing::instrument(skip_all, fields(slot = %slot, signature = %signature))]
    fn decode(
        &self,
        tx: SubscribeUpdateTransactionInfo,
        slot: Slot,
        signature: Signature,
    ) -> Result<Vec<DecodedEvent>, PartialDecode<DecodedEvent>> {
        // `meta` carries whether the transaction succeeded, so without it there is
        // no way to tell, and emitting events for a transaction that may have
        // reverted is the failure this guard exists to prevent. Dead-letter it
        // instead of skipping: replay re-fetches by signature, and
        // `getTransaction` returns the meta.
        let Some(meta) = tx.meta.as_ref() else {
            tracing::warn!("transaction update without meta");
            return Err(PartialDecode { events: Vec::new() });
        };

        // A failed Solana transaction rolls back every account write, so no
        // state-changing event may be emitted for it. Failed transactions are
        // delivered on purpose (the revert is an attribution signal), and no
        // event models the revert.
        if meta.err.is_some() {
            tracing::debug!("transaction reverted, skipping");
            return Ok(Vec::new());
        }

        let instructions =
            relevant_instructions(&tx, &self.settlement_program, &self.solflow_program);
        if instructions.is_empty() {
            return Ok(Vec::new());
        }

        // `relevant_instructions` reconstructs the account list internally to
        // resolve program ids; rebuild it once here so the decode can resolve
        // account indices to pubkeys too.
        let ctx = TxContext {
            slot,
            signature,
            account_keys: build_account_keys(&tx),
            post_token_balances: tx
                .meta
                .map(|meta| meta.post_token_balances)
                .unwrap_or_default(),
        };

        let (settlement, solflow): (Vec<ResolvedInstruction>, Vec<ResolvedInstruction>) =
            instructions
                .into_iter()
                .partition(|instruction| instruction.program_id == self.settlement_program);

        // TODO: resolve order PDAs against persisted order rows. Without a
        // resolver backend nothing resolves, so `SettlementFinalized` events
        // carry the tx-level fields with empty trades.
        let (events, decode_failed) = match decode_settlement(&settlement, &ctx, |_order_pda| None)
        {
            Ok(events) => (events, false),
            Err(PartialDecode { events }) => (events, true),
        };

        for instruction in &solflow {
            self.decode_solflow(instruction);
        }

        let events = events.into_iter().map(DecodedEvent::Settlement).collect();
        if decode_failed {
            return Err(PartialDecode { events });
        }
        Ok(events)
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

/// One slot's accumulated output, flushed when the slot is over.
struct SlotBuffer {
    slot: Slot,
    events: Vec<DecodedEvent>,
    /// Failed transactions and the slot each belongs to. A late transaction
    /// lands in a newer slot's buffer, so the buffer's slot would be wrong
    /// for its dead-letter row.
    dead_letters: Vec<(Signature, Slot)>,
}

impl SlotBuffer {
    fn new(slot: Slot) -> Self {
        Self {
            slot,
            events: Vec::new(),
            dead_letters: Vec::new(),
        }
    }
}

/// The events that did decode from a transaction in which at least one
/// instruction failed to decode. The caller persists them and dead-letters
/// the transaction for replay.
#[derive(Debug, PartialEq, Eq)]
struct PartialDecode<T> {
    events: Vec<T>,
}

/// Decode the settlement-program instructions of one transaction into domain
/// events. `Err` means at least one instruction failed to decode, and carries
/// the events that did decode so the caller has to acknowledge the failure to
/// get at them.
///
/// Pure: every tx-level input arrives through `ctx`, and any order field the
/// `BeginSettle` wire does not carry is resolved through `resolve_order` (keyed
/// on the order PDA), so tests can inject a canned map. `instructions` must be
/// the transaction's settlement-program instructions, in execution order.
fn decode_settlement(
    instructions: &[ResolvedInstruction],
    ctx: &TxContext,
    resolve_order: impl Fn(&Pubkey) -> Option<ResolvedOrder>,
) -> Result<Vec<SettlementEvent>, PartialDecode<SettlementEvent>> {
    let mut events = Vec::new();
    let mut decode_failed = false;

    // Instructions decodable on their own, without tx-level pairing.
    for instruction in instructions {
        let Ok((discriminator, _)) = recover_discriminator(&instruction.data) else {
            decode_failed = true;
            // Warn, not debug: this dead-letters the transaction, so it needs to
            // be findable in the logs alongside the row.
            tracing::warn!(
                instruction_index = instruction.instruction_index,
                err = %DecodeError::UnknownDiscriminator,
                "settlement instruction with an unknown discriminator, skipping"
            );
            continue;
        };
        // A landed (non-reverted) transaction carries valid instruction data, so
        // a decode failure here means a decoder bug or an unannounced program
        // layout change, not a normal case. Surface it as a warning and set the
        // failure flag so the transaction is dead-lettered.
        let decoded = match discriminator {
            SettlementInstruction::CreateOrder => {
                decode_order_created(instruction, &ctx.account_keys).map(|event| vec![event])
            }
            SettlementInstruction::CreateBuffer => {
                decode_buffers_created(instruction, &ctx.account_keys)
            }
            // Paired below, once both halves of the settlement are in hand.
            SettlementInstruction::BeginSettle | SettlementInstruction::FinalizeSettle => {
                Ok(Vec::new())
            }
            // No domain event: `Initialize` bootstraps the program state.
            // TODO: map `ReclaimOrder` to `OrderClosed`.
            SettlementInstruction::Initialize | SettlementInstruction::ReclaimOrder => {
                Ok(Vec::new())
            }
        };
        match decoded {
            Ok(decoded_events) => events.extend(decoded_events),
            Err(err) => {
                decode_failed = true;
                tracing::warn!(
                        instruction_index = instruction.instruction_index,
                    %err,
                    "failed to decode settlement instruction"
                );
            }
        }
    }

    // A `BeginSettle` plus the `FinalizeSettle` it names make one
    // `SettlementFinalized`; pairing needs the whole transaction.
    events.extend(decode_settlements_finalized(
        instructions,
        ctx,
        &resolve_order,
        &mut decode_failed,
    ));
    if decode_failed {
        return Err(PartialDecode { events });
    }
    Ok(events)
}

/// `CreateOrder` -> `OrderCreated`. The parser recovers the encoded order
/// intent and the `created_by` account. The intent's hash is the order UID,
/// and the intent carries the owner.
fn decode_order_created(
    instruction: &ResolvedInstruction,
    account_keys: &[Pubkey],
) -> Result<SettlementEvent, DecodeError> {
    let mut accounts = instruction_account_keys(instruction, account_keys)?;
    let input = CreateOrderInput::parse(&instruction.data, &mut accounts)
        .map_err(|err| DecodeError::SchemaMismatch(err.to_string()))?;
    let (intent, uid) = EncodedOrderIntent::decode_and_hash(&input.intent_bytes)
        .map_err(|err| DecodeError::SchemaMismatch(err.to_string()))?;
    Ok(SettlementEvent::OrderCreated {
        order_uid: OrderUid(uid.to_bytes()),
        owner: to_sdk_pubkey(intent.owner),
        created_by: *input.created_by,
    })
}

/// `CreateBuffer` -> one `BufferCreated` per created buffer. The parser groups
/// the trailing accounts into `[buffer_pda, mint]` pairs, and the event's
/// token is each pair's mint.
fn decode_buffers_created(
    instruction: &ResolvedInstruction,
    account_keys: &[Pubkey],
) -> Result<Vec<SettlementEvent>, DecodeError> {
    let mut accounts = instruction_account_keys(instruction, account_keys)?;
    let input = CreateBufferInput::parse(&instruction.data, &mut accounts)
        .map_err(|err| DecodeError::SchemaMismatch(err.to_string()))?;
    Ok(input
        .buffers
        .iter()
        .map(|pair| SettlementEvent::BufferCreated { token: pair[1] })
        .collect())
}

/// Pair each `BeginSettle` with the `FinalizeSettle` it names and emit one
/// `SettlementFinalized` per pair.
///
/// Pairing is by index: a parsed `BeginSettle` carries the top-level
/// instruction index of its `FinalizeSettle` (`finalize_ix_index`), which must
/// match a `FinalizeSettle` present in the same transaction. It is independent
/// of the two instructions' relative order. A pair that cannot be decoded (a
/// parse failure, unresolvable account keys, or a `BeginSettle` whose named
/// `FinalizeSettle` is missing) sets `decode_failed` and emits nothing.
fn decode_settlements_finalized(
    instructions: &[ResolvedInstruction],
    ctx: &TxContext,
    resolve_order: &impl Fn(&Pubkey) -> Option<ResolvedOrder>,
    decode_failed: &mut bool,
) -> Vec<SettlementEvent> {
    // The solver is the transaction fee payer: the first account key, which
    // Solana guarantees is the signer that submitted the transaction.
    let Some(&solver) = ctx.account_keys.first() else {
        return Vec::new();
    };

    let mut events = Vec::new();
    'process_instructions: for begin in instructions {
        let Ok((SettlementInstruction::BeginSettle, _)) = recover_discriminator(&begin.data) else {
            continue;
        };
        let mut begin_accounts = match instruction_account_keys(begin, &ctx.account_keys) {
            Ok(accounts) => accounts,
            Err(err) => {
                *decode_failed = true;
                tracing::warn!(
                        instruction_index = begin.instruction_index,
                    %err,
                    "BeginSettle account resolution failed, skipping"
                );
                continue;
            }
        };
        let begin_input = match BeginSettleInput::parse(&begin.data, &mut begin_accounts) {
            Ok(input) => input,
            Err(err) => {
                *decode_failed = true;
                tracing::warn!(
                        instruction_index = begin.instruction_index,
                    %err,
                    "BeginSettle did not match the expected layout, skipping"
                );
                continue;
            }
        };

        // The named `FinalizeSettle` must actually be present in this tx.
        let Some(finalize) = instructions.iter().find(|instruction| {
            instruction.instruction_index == u32::from(begin_input.finalize_ix_index)
                && matches!(
                    recover_discriminator(&instruction.data),
                    Ok((SettlementInstruction::FinalizeSettle, _))
                )
        }) else {
            *decode_failed = true;
            tracing::warn!(
                instruction_index = begin.instruction_index,
                finalize_ix_index = begin_input.finalize_ix_index,
                "BeginSettle without a paired FinalizeSettle in the tx, skipping"
            );
            continue;
        };
        let mut finalize_accounts = match instruction_account_keys(finalize, &ctx.account_keys) {
            Ok(accounts) => accounts,
            Err(err) => {
                *decode_failed = true;
                tracing::warn!(
                        instruction_index = finalize.instruction_index,
                    %err,
                    "FinalizeSettle account resolution failed, skipping"
                );
                continue;
            }
        };
        let finalize_input =
            match FinalizeSettleInput::parse(&finalize.data, &mut finalize_accounts) {
                Ok(input) => input,
                Err(err) => {
                    *decode_failed = true;
                    tracing::warn!(
                                instruction_index = finalize.instruction_index,
                        %err,
                        "FinalizeSettle did not match the expected layout, skipping"
                    );
                    continue;
                }
            };
        // Orders and finalize pushes are positionally aligned: `BeginSettle`
        // enforces exactly one push per order, both sorted by order PDA, so
        // order `i` is paid by push `i`. A count mismatch breaks that
        // invariant, so the pair cannot be decoded.
        let order_count = begin_input.orders.iter().count();
        let push_count = finalize_input.pushes.iter().count();
        if order_count != push_count {
            *decode_failed = true;
            tracing::warn!(
                instruction_index = begin.instruction_index,
                order_count,
                push_count,
                "order and push counts differ, skipping the settlement pair"
            );
            continue;
        }

        let mut trades = Vec::with_capacity(order_count);
        for (order, amount_received_delta) in begin_input
            .orders
            .iter()
            .zip(finalize_input.pushes.iter().map(|push| push.amount))
        {
            let Some(resolved) = resolve_order(order.order_pda) else {
                continue;
            };
            // Sell-side pull total, little-endian on the wire. An overflowing
            // sum cannot be a real settlement, so it invalidates the pair.
            let sum = order.amounts.iter().try_fold(0u64, |acc, amount| {
                acc.checked_add(u64::from_le_bytes(*amount))
            });
            let Some(amount_withdrawn_delta) = sum else {
                *decode_failed = true;
                tracing::warn!(
                    instruction_index = begin.instruction_index,
                    "pull amounts overflow u64, skipping the settlement pair"
                );
                continue 'process_instructions;
            };
            trades.push(TradeDelta {
                order_uid: resolved.order_uid,
                amount_withdrawn_delta,
                amount_received_delta,
                order_fulfilled: resolved.order_fulfilled,
            });
        }

        events.push(SettlementEvent::SettlementFinalized {
            auction_id: begin_input.auction_id,
            solver,
            tx_signature: ctx.signature,
            slot: ctx.slot,
            trades,
        });
    }
    events
}

/// Resolve an instruction's account-list indices to their pubkeys, in order, so
/// the interface parser can read them positionally. Fails if any index is out
/// of range against the transaction's account list.
fn instruction_account_keys(
    instruction: &ResolvedInstruction,
    account_keys: &[Pubkey],
) -> Result<Vec<Pubkey>, DecodeError> {
    instruction
        .accounts
        .iter()
        .map(|&index| {
            account_keys.get(usize::from(index)).copied().ok_or(
                DecodeError::AccountIndexOutOfRange {
                    index,
                    len: account_keys.len(),
                },
            )
        })
        .collect()
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
