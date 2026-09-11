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
    /// A confirmed slot-status message. The stream delivers a slot's
    /// transactions before its confirmed status, so this message means every
    /// buffered transaction at or below the slot is complete and can flush.
    Confirmed {
        /// The slot the status message reports confirmed.
        slot: Slot,
    },
    /// A finalized slot-status message. Advances the finalized watermark:
    /// rows at or below it can no longer roll back.
    Finalized {
        /// The slot the status message reports finalized.
        slot: Slot,
    },
}
