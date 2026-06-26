#![expect(dead_code)]
//! Dead-letter types: events that failed to persist and were diverted to
//! `solana.dead_letter` for operator follow-up.

use {
    crate::types::{Signature, slot::Slot},
    bytes::Bytes,
};

/// A decoded event whose write to `solana.*` failed and was diverted to
/// `solana.dead_letter`.
#[derive(Debug, Clone)]
pub(crate) struct DeadLetterEntry {
    /// Slot the event was observed at.
    pub slot: Slot,
    /// Transaction signature, if the failure was per-transaction.
    pub signature: Option<Signature>,
    /// Why the event landed in the dead-letter table.
    pub reason: DeadLetterReason,
    /// Original raw bytes for replay.
    pub raw_bytes: Bytes,
}

/// Why a row landed in the dead-letter table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeadLetterReason {
    /// Decoder received both halves but couldn't parse them.
    DecoderError,
    /// Watchdog gave up: account-update half never arrived.
    AccountUpdateMissing,
    /// Watchdog gave up: transaction-update half never arrived.
    TxUpdateMissing,
    /// Settlement landed but no `proposed_solutions` row matched.
    SolutionUidUnmatchable,
}

impl DeadLetterReason {
    /// String label used in `solana.dead_letter.reason`.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::DecoderError => "decoder_error",
            Self::AccountUpdateMissing => "account_update_missing",
            Self::TxUpdateMissing => "tx_update_missing",
            Self::SolutionUidUnmatchable => "solution_uid_unmatchable",
        }
    }
}
