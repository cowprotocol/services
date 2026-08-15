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
    /// A slot-status message. Lets the decoder flush a buffered slot without
    /// waiting for the next tracked transaction, which can be arbitrarily far
    /// away. Only slots at the transaction stream's commitment may be
    /// forwarded, an earlier-commitment slot would flush a buffer whose
    /// transactions are still in flight.
    Slot {
        /// The slot the status message reports.
        slot: Slot,
    },
}
