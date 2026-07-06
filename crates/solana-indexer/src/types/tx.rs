#![expect(dead_code)]
//! Per-transaction helper types used by the decoder.
//!
//! These are decoder-side views produced by walking a
//! `SubscribeUpdateTransactionInfo` message. They are not present in
//! `solana-sdk` or `solana-message` because the upstream types carry raw
//! `program_id_index: u8` values, while these views resolve that index
//! against the reconstructed `account_keys` list and tag each instruction
//! with its position in the transaction.

use {
    crate::types::{Signature, slot::Slot, wire::TokenBalance},
    bytes::Bytes,
    solana_sdk::pubkey::Pubkey,
};

/// A single instruction after resolving `program_id_index` against the full
/// account list.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedInstruction {
    /// Resolved program id.
    pub program_id: Pubkey,
    /// Raw instruction data.
    pub data: Bytes,
    /// Account indices into the reconstructed account list.
    pub accounts: Vec<u8>,
    /// Top-level instruction index. For a CPI, the top-level instruction it
    /// runs under.
    pub instruction_index: u32,
    /// Path to this instruction within the top-level instruction's CPI tree,
    /// one sibling position per nesting level: empty for a top-level
    /// instruction, `[0]` for its first CPI, `[0, 1]` for the second CPI made
    /// by that first CPI. Reconstructed from `stack_height`.
    pub inner_ix_path: Vec<u8>,
}

/// Per-decode-pass context: the reconstructed account list, the slot, and the
/// transaction signature.
#[derive(Debug, Clone)]
pub(crate) struct TxContext {
    /// Slot the transaction was observed at.
    pub slot: Slot,
    /// Transaction signature.
    pub signature: Signature,
    /// Reconstructed account list (`message.account_keys` ⊕
    /// `meta.loaded_writable_addresses` ⊕
    /// `meta.loaded_readonly_addresses`).
    pub account_keys: Vec<Pubkey>,
    /// Post-execution token balances, copied from `meta.post_token_balances`.
    /// The SolFlow `OrderCreated` branch reads the wSOL balance on the
    /// custodial PDA here.
    pub post_token_balances: Vec<TokenBalance>,
}
