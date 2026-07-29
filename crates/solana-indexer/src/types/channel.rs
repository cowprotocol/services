//! Message types passed over the internal channel.
//!
//! The ingester pushes [`StreamUpdate`] into the channel to the decoder.

use crate::types::{Signature, slot::Slot, wire::SubscribeUpdateTransactionInfo};

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
}
