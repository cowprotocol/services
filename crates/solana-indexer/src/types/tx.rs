#![expect(dead_code)]
//! Per-transaction helper types used by the decoder.
//!
//! These are decoder-side views produced by walking a
//! `SubscribeUpdateTransactionInfo` message. They are not present in
//! `solana-sdk` or `solana-message` because the upstream types carry raw
//! `program_id_index: u8` values, while these views resolve that index
//! against the reconstructed `account_keys` list and tag each instruction
//! with its source (`is_inner`) and position (`ix_index`).

use {
    crate::types::{
        Signature,
        Slot,
        wire::{SubscribeUpdateAccountInfo, TokenBalance},
    },
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
    /// Index of this instruction within the transaction (outer or
    /// inner).
    pub ix_index: u8,
    /// `true` for CPIs (inner instructions).
    pub is_inner: bool,
}

/// Per-decode-pass context: the reconstructed account list, the slot, the
/// transaction signature, and (when both halves have arrived) the joined
/// account-update snapshot.
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
    /// Account-update snapshot, if both halves have arrived.
    pub account_snapshot: Option<SubscribeUpdateAccountInfo>,
}
