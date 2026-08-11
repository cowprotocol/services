#![expect(dead_code)]
//! Error types used across the indexer's domain.

use {crate::types::slot::Slot, thiserror::Error};

/// Failures surfaced from the decoder.
#[derive(Debug, Error)]
pub(crate) enum DecodeError {
    /// The discriminator byte(s) at the start of the instruction data did not
    /// match any known instruction on either program.
    #[error("unknown instruction discriminator")]
    UnknownDiscriminator,
    /// An account index did not resolve against the transaction's account list,
    /// which includes the ALT (Address Lookup Table) loaded addresses.
    #[error("account index {index} out of range for {len} account keys")]
    AccountIndexOutOfRange { index: u8, len: usize },
    /// The instruction was recognised but its schema did not match the
    /// on-chain layout. Carries the parser's error rendered as text, which
    /// names the failed check. Nothing branches on it, and the interface does
    /// not re-export its error type.
    #[error("schema mismatch")]
    SchemaMismatch,
}

/// Failures surfaced from the persistence boundary.
#[derive(Debug, Error)]
pub(crate) enum PersistenceError {
    /// The SQL `ON CONFLICT` clause rejected the write (e.g. watermark
    /// regression).
    #[error("persistence conflict")]
    Conflict,
    /// The persistence layer is temporarily unavailable (e.g. connection lost,
    /// pool exhausted). The caller is expected to retry.
    #[error("persistence unavailable")]
    Unavailable,
    /// The database rejected or failed a statement.
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

/// Failures surfaced from the stream boundary.
#[derive(Debug, Error)]
pub(crate) enum StreamError {
    /// The stream has been disconnected by the server.
    #[error("stream disconnected")]
    Disconnected,
    /// The internal mpsc send timed out (backpressure on the decoder).
    #[error("stream send timeout")]
    SendTimeout,
    /// The resume slot is outside the provider's replay window. The caller
    /// should reset `from_slot` to the latest chain slot minus the replay
    /// window, record the lost range, and retry the subscription.
    #[error(
        "replay window exceeded: attempted slot {attempted_slot}, earliest replayable \
         {earliest_replayable_slot}"
    )]
    ReplayWindowExceeded {
        /// The slot the subscriber attempted to resume from.
        attempted_slot: Slot,
        /// The earliest slot the provider can still serve.
        earliest_replayable_slot: Slot,
    },
}
