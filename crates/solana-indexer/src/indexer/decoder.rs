#![expect(dead_code)]
//! The decoder pulls `StreamUpdate`s from the ingester, decodes
//! settlement-program and SolFlow transactions, joins account-update snapshots,
//! and persists typed events.

// TODO: `run` is unimplemented. The dispatch and persist steps that consume the
// resolved instructions below are not wired up yet.

use {
    crate::{
        persistence::Persistence,
        types::{
            Signature,
            channel::{PartialHalf, StreamUpdate},
            errors::PersistenceError,
            slot::Slot,
            tx::ResolvedInstruction,
            wire::SubscribeUpdateTransactionInfo,
        },
    },
    bytes::Bytes,
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

/// The transaction's full account list that instruction indices resolve
/// against: static keys, then ALT-loaded writable addresses, then readonly, in
/// that order. A wrong-length key becomes the zero pubkey to keep the indices
/// aligned, so it matches no tracked program.
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
    /// Position within the top-level instruction's inner list, or `None` for a
    /// top-level instruction.
    inner_index: Option<u32>,
    /// Index of the invoked program in the account list.
    program_id_index: u32,
    /// Account-list indices of the accounts the instruction touches.
    account_indices: &'a [u8],
    /// Raw instruction data.
    data: &'a [u8],
}

impl RawInstruction<'_> {
    /// Resolve the program against `account_keys`, keeping the instruction only
    /// if that program is the settlement or SolFlow program. Account indices
    /// are carried through unresolved. Returns `None` if the program is
    /// untracked or its index is out of range.
    fn resolve(
        &self,
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
            inner_index: self.inner_index,
        })
    }
}

/// Resolve every instruction against `account_keys` and keep only those whose
/// program is the settlement or SolFlow program. Walks top-level instructions
/// then inner/CPI ones, where settlement is often reached only as a CPI.
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
    // TODO: top-level instructions come before inner ones here, which is not the
    // on-chain execution order. Revisit if ordering across the two matters.
    top_level
        .chain(inner)
        .filter_map(|raw| raw.resolve(&account_keys, settlement_program, solflow_program))
        .collect()
}

#[cfg(test)]
mod tests;
