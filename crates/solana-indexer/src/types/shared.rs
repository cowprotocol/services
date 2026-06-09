//! Types shared across the internal compoents of this crate.

use crate::types::{
    Signature,
    wire::{SubscribeUpdateAccountInfo, SubscribeUpdateTransactionInfo},
};

/// From `Ingester` → `Decoder`.
///
/// One multiplexed wire message, tagged with the slot the message was observed
/// at. The org file names the channel payload "Event"; the spec defines that
/// type as `StreamUpdate`, and that is what this crate uses.
#[derive(Debug, Clone)]
pub enum StreamUpdate {
    /// A transaction-update slot message.
    Tx {
        /// Slot the message was observed at.
        slot: u64,
        /// Transaction signature.
        signature: Signature,
        /// Wire message body.
        inner: Box<SubscribeUpdateTransactionInfo>,
    },
    /// An account-update slot message.
    Account {
        /// Slot the message was observed at.
        slot: u64,
        /// Optional transaction signature linking the write back to its
        /// originating transaction.
        txn_signature: Option<Signature>,
        /// Wire message body.
        inner: Box<SubscribeUpdateAccountInfo>,
    },
}

/// Key for the shared decoder↔watchdog partials map: the `(slot, signature)`
/// pair identifying which on-chain event a `PartialEvent` belongs to.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct PartialEventKey(pub u64, pub Signature);

/// One half of a paired on-chain event, recorded by the decoder when only
/// one of the two matching `StreamUpdate` messages has been observed for a
/// given `PartialEventKey`.
///
/// The other half is expected to arrive shortly; until it does, the entry
/// lives in the shared decoder↔watchdog map. The watchdog scans the map and
/// dead-letters any partial that has aged out (the matching half never
/// arrived within the slot window), using the variant to report which half
/// was missing.
///
/// Both components hold a clone of the same
/// `Arc<DashMap<PartialEventKey, PartialEvent>>`, so there is no message
/// passing between them — the watchdog simply reads what the decoder wrote.
#[derive(Debug, Clone)]
pub enum PartialEvent {
    /// Transaction-update half.
    Tx(Box<SubscribeUpdateTransactionInfo>),
    /// Account-update half.
    Account(Box<SubscribeUpdateAccountInfo>),
}
