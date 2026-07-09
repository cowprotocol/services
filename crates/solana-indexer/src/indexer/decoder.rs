#![expect(dead_code)]
//! The decoder pulls `StreamUpdate`s from the ingester, decodes
//! settlement-program and SolFlow transactions, and persists typed events.

// TODO: `decode_settlement`/`decode_solflow` and the persist path are stubbed.
// `run` drains the channel and routes each transaction's tracked instructions
// to those stubs.

use {
    crate::{
        persistence::Persistence,
        types::{
            channel::StreamUpdate,
            errors::PersistenceError,
            tx::ResolvedInstruction,
            wire::SubscribeUpdateTransactionInfo,
        },
    },
    bytes::Bytes,
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

    /// Main loop. Drains the channel and routes each transaction's tracked
    /// instructions to their per-program decoders. Returns when the ingester
    /// drops the sender.
    pub async fn run(&mut self) -> Result<(), PersistenceError> {
        while let Some(update) = self.rx.recv().await {
            let StreamUpdate::Tx { inner, .. } = update;
            self.decode(&inner);
        }
        Ok(())
    }

    /// Route each tracked instruction in the transaction to its per-program
    /// decoder.
    fn decode(&self, tx: &SubscribeUpdateTransactionInfo) {
        for instruction in
            relevant_instructions(tx, &self.settlement_program, &self.solflow_program)
        {
            if instruction.program_id == self.settlement_program {
                self.decode_settlement(&instruction);
            } else {
                self.decode_solflow(&instruction);
            }
        }
    }

    /// TODO: decode the settlement instruction data into typed events.
    fn decode_settlement(&self, instruction: &ResolvedInstruction) {
        tracing::debug!(
            instruction_index = instruction.instruction_index,
            "settlement instruction decode not implemented"
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
