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
/// and accounts resolved against the transaction's full account list. This is
/// what the per-program dispatch consumes.
pub(crate) struct RelevantInstruction {
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

/// §6.3.1.b/c: resolve every instruction in the transaction against
/// `account_keys` and keep only those whose program is the settlement or
/// SolFlow program. Walks top-level (`message.instructions`) then inner/CPI
/// (`meta.inner_instructions[_].instructions`). CPIs into the settlement
/// program appear only in the inner list. An instruction whose program or
/// account indices fall outside the account list is malformed and dropped.
fn relevant_instructions(
    tx: &SubscribeUpdateTransactionInfo,
    settlement_program: &Pubkey,
    solflow_program: &Pubkey,
) -> Vec<RelevantInstruction> {
    let account_keys = build_account_keys(tx);
    let resolve = |program_id_index: u32,
                   account_indices: &[u8],
                   data: &[u8]|
     -> Option<RelevantInstruction> {
        let program = *account_keys.get(program_id_index as usize)?;
        if program != *settlement_program && program != *solflow_program {
            return None;
        }
        let accounts = account_indices
            .iter()
            .map(|&index| account_keys.get(index as usize).copied())
            .collect::<Option<Vec<_>>>()?;
        Some(RelevantInstruction {
            program,
            accounts,
            data: data.to_vec(),
        })
    };

    let top_level = tx
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.message.as_ref())
        .map(|message| message.instructions.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|ix| resolve(ix.program_id_index, &ix.accounts, &ix.data));
    let inner = tx
        .meta
        .as_ref()
        .map(|meta| meta.inner_instructions.as_slice())
        .unwrap_or_default()
        .iter()
        .flat_map(|group| group.instructions.iter())
        .filter_map(|ix| resolve(ix.program_id_index, &ix.accounts, &ix.data));
    top_level.chain(inner).collect()
}

#[cfg(test)]
mod tests;
