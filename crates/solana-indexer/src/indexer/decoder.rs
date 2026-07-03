#![expect(dead_code)]
//! The decoder pulls `StreamUpdate`s from the ingester, decodes
//! settlement-program and SolFlow transactions, joins account-update snapshots,
//! and persists typed events.

// TODO: This file only declares the component skeleton. The `run` body is
// `unimplemented!`; the dispatch logic and persist path arrive in a later
// change.

use {
    crate::{
        persistence::Persistence,
        types::{
            Signature,
            channel::{PartialHalf, StreamUpdate},
            errors::PersistenceError,
            slot::Slot,
            wire::SubscribeUpdateTransactionInfo,
        },
    },
    dashmap::DashMap,
    solana_sdk::pubkey::Pubkey,
    std::sync::Arc,
    tokio::sync::mpsc::Receiver,
};

/// Decoder component.
///
/// The watchdog holds a clone of the same `partials` map, so the two operate on
/// the same concurrent map without any message passing between them.
pub(crate) struct Decoder {
    /// Persistence layer.
    pub persistence: Persistence,

    /// Incoming `StreamUpdate` from the ingester.
    pub rx: Receiver<StreamUpdate>,

    /// Shared in-memory map of partial events keyed by `(slot, signature)`,
    /// holding either-half events waiting for their pair. The watchdog holds a
    /// clone of this `Arc`.
    pub partials: Arc<DashMap<(Slot, Signature), PartialHalf>>,

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
        partials: Arc<DashMap<(Slot, Signature), PartialHalf>>,
        settlement_program: Pubkey,
        solflow_program: Pubkey,
    ) -> Self {
        Self {
            persistence,
            rx,
            partials,
            settlement_program,
            solflow_program,
        }
    }

    /// Main loop. Pulls `StreamUpdate` from the receiver, runs the decode
    /// pipeline, persists, and records partial events in the shared map for the
    /// watchdog to read.
    pub async fn run(&mut self) -> Result<(), PersistenceError> {
        unimplemented!()
    }
}

/// An instruction that targets a program the decoder tracks, with its program
/// and accounts resolved against the transaction's full account list, and its
/// position in the transaction. This is what the per-program dispatch consumes.
pub(crate) struct RelevantInstruction {
    /// Top-level instruction index. For a CPI, the index of the top-level
    /// instruction it runs under. PR7 maps `(instruction_index, inner_index)`
    /// to the `solana.trades` `(instruction_index, inner_ix_path)` key.
    pub instruction_index: u32,
    /// Position within the top-level instruction's inner list, or `None` for a
    /// top-level instruction.
    pub inner_index: Option<u32>,
    /// Resolved program id: the settlement or SolFlow program.
    pub program: Pubkey,
    /// The instruction's accounts, resolved to pubkeys in order.
    pub accounts: Vec<Pubkey>,
    /// Raw instruction data, decoded by the per-program dispatch.
    pub data: Vec<u8>,
}

/// §6.3.1.a: the transaction's full account list - `message.account_keys` then
/// the ALT-loaded writable then readonly addresses, concatenated in that fixed
/// order. Versioned transactions put ALT-loaded accounts in the latter two
/// fields, so an instruction's `program_id_index` only resolves against the
/// concatenation.
///
/// A wrong-length key becomes the zero pubkey to keep index alignment. It
/// cannot match a tracked program, so any instruction naming it as its program
/// is dropped by [`relevant_instructions`].
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

/// A transaction instruction (top-level or inner/CPI) paired with its position
/// in the transaction, before its program and account indices are resolved to
/// pubkeys.
struct RawInstruction<'a> {
    /// Top-level instruction index. For a CPI, the index of the top-level
    /// instruction it runs under (`InnerInstructions.index`).
    instruction_index: u32,
    /// Position within that top-level instruction's inner list, or `None` for a
    /// top-level instruction.
    inner_index: Option<u32>,
    /// Index into the account list identifying the program invoked.
    program_id_index: u32,
    /// Indices into the account list of the accounts the instruction touches.
    account_indices: &'a [u8],
    /// Raw instruction data (discriminator + payload).
    data: &'a [u8],
}

impl RawInstruction<'_> {
    /// Resolve against `account_keys`, keeping the instruction only if its
    /// program is the settlement or SolFlow program. Returns `None` when the
    /// program is untracked, or when the program or an account index falls
    /// outside the account list.
    fn resolve(
        &self,
        account_keys: &[Pubkey],
        settlement_program: &Pubkey,
        solflow_program: &Pubkey,
    ) -> Option<RelevantInstruction> {
        let program = *account_keys.get(self.program_id_index as usize)?;
        if program != *settlement_program && program != *solflow_program {
            return None;
        }
        // TODO(PR7): here the program matched a tracked program but an account
        // index is out of range - a malformed settlement we identified and then
        // drop with no trace. Decide whether to dead-letter or log instead.
        let accounts = self
            .account_indices
            .iter()
            .map(|&index| account_keys.get(index as usize).copied())
            .collect::<Option<Vec<_>>>()?;
        Some(RelevantInstruction {
            instruction_index: self.instruction_index,
            inner_index: self.inner_index,
            program,
            accounts,
            data: self.data.to_vec(),
        })
    }
}

/// §6.3.1.b/c: resolve every instruction in the transaction against
/// `account_keys` and keep only those whose program is the settlement or
/// SolFlow program. Walks top-level (`message.instructions`) then inner/CPI
/// (`meta.inner_instructions[_].instructions`). CPIs into the settlement
/// program appear only in the inner list.
fn relevant_instructions(
    tx: &SubscribeUpdateTransactionInfo,
    settlement_program: &Pubkey,
    solflow_program: &Pubkey,
) -> Vec<RelevantInstruction> {
    let account_keys = build_account_keys(tx);
    let top_level = tx
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.message.as_ref())
        .map(|message| message.instructions.as_slice())
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(index, ix)| RawInstruction {
            instruction_index: index as u32,
            inner_index: None,
            program_id_index: ix.program_id_index,
            account_indices: &ix.accounts,
            data: &ix.data,
        });
    let inner = tx
        .meta
        .as_ref()
        .map(|meta| meta.inner_instructions.as_slice())
        .unwrap_or_default()
        .iter()
        .flat_map(|group| {
            group
                .instructions
                .iter()
                .enumerate()
                .map(move |(offset, ix)| RawInstruction {
                    instruction_index: group.index,
                    inner_index: Some(offset as u32),
                    program_id_index: ix.program_id_index,
                    account_indices: &ix.accounts,
                    data: &ix.data,
                })
        });
    // TODO(PR7): top-level instructions are emitted before inner/CPI ones, not
    // in on-chain execution order (CPIs run interleaved with their parent).
    // Confirm PR7 keys off (instruction_index, inner_index) rather than list
    // order, or interleave here if it needs execution order.
    top_level
        .chain(inner)
        .filter_map(|raw| raw.resolve(&account_keys, settlement_program, solflow_program))
        .collect()
}

#[cfg(test)]
mod tests;
