//! Error types used across the indexer's domain.

use {crate::types::Slot, thiserror::Error};

/// Failures surfaced from the decoder.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    /// The discriminator byte(s) at the start of the instruction data did not
    /// match any known instruction on either program.
    #[error("unknown instruction discriminator")]
    UnknownDiscriminator,
    /// The ALT (Address Lookup Table) loaded-address list could not be resolved
    /// against the full account list.
    #[error("alt resolution failed")]
    AltResolutionFailed,
    /// The instruction was recognised but its schema did not match the on-chain
    /// layout.
    #[error("schema mismatch")]
    SchemaMismatch,
}

/// Failures surfaced from the persistence boundary.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoreError {
    /// The SQL `ON CONFLICT` clause rejected the write (e.g. watermark
    /// regression).
    #[error("store conflict")]
    Conflict,
    /// The store is temporarily unavailable (e.g. connection lost, pool
    /// exhausted). The caller is expected to retry.
    #[error("store unavailable")]
    Unavailable,
}

/// Failures surfaced from the stream boundary.
#[derive(Debug, Error)]
pub enum StreamError {
    /// The stream has been disconnected by the server.
    #[error("stream disconnected")]
    Disconnected,
    /// The internal mpsc send timed out (backpressure on the decoder).
    #[error("stream send timeout")]
    SendTimeout,
    /// The resume slot is outside the provider's replay window. The caller
    /// should reset `from_slot` to `LATEST_CHAIN_SLOT − replay_window`,
    /// record the lost range, and retry the subscription.
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
