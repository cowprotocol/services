#![expect(dead_code)]
//! Message types passed over the internal channels.
//!
//! The ingester pushes [`StreamUpdate`] into the channel to the decoder; the
//! decoder pushes [`PartialEvent`] / [`PartialHalf`] to the partial-event
//! watchdog.

use crate::types::{
    Signature,
    Slot,
    wire::{SubscribeUpdateAccountInfo, SubscribeUpdateTransactionInfo},
};

/// From `Ingester` → `Decoder`.
///
/// One multiplexed wire message, tagged with the slot the message was observed
/// at.
#[derive(Debug, Clone)]
pub(crate) enum StreamUpdate {
    /// A transaction-update slot message.
    Tx {
        /// Slot the message was observed at.
        slot: Slot,
        /// Transaction signature.
        signature: Signature,
        /// Wire message body.
        inner: Box<SubscribeUpdateTransactionInfo>,
    },
    /// An account-update slot message.
    Account {
        /// Slot the message was observed at.
        slot: Slot,
        /// Optional signature linking the write back to its originating
        /// transaction.
        txn_signature: Option<Signature>,
        /// Wire message body.
        inner: Box<SubscribeUpdateAccountInfo>,
    },
}

/// From `Decoder` → `PartialEventWatchdog`.
///
/// The watchdog holds incomplete `(slot, signature)` pairs until both halves
/// arrive; each delivery carries the half that just landed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PartialEvent {
    /// Slot the partial was observed at.
    pub slot: Slot,
    /// Transaction signature the partial corresponds to.
    pub signature: Signature,
}

/// One of the two halves a [`StreamUpdate`] can produce.
///
/// The decoder pushes one `PartialEvent` per `StreamUpdate` it processes; the
/// watchdog uses the `(slot, signature)` key to match pairs.
#[derive(Debug, Clone)]
pub(crate) enum PartialHalf {
    /// Transaction-update half.
    Tx(Box<SubscribeUpdateTransactionInfo>),
    /// Account-update half.
    Account(Box<SubscribeUpdateAccountInfo>),
}
