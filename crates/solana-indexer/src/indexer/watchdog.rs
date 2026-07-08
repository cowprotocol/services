#![expect(dead_code)]
//! The partial-event watchdog.

// TODO: This file only declares the component skeleton. The `run` body is
// `unimplemented!`; the lag-detection and dead-letter logic arrive in a later
// change.

use {
    crate::{
        persistence::Persistence,
        types::{Signature, channel::PartialHalf, errors::PersistenceError, slot::Slot},
    },
    dashmap::DashMap,
    std::sync::Arc,
};

/// Partial-event watchdog component.
///
/// The watchdog holds a view of the partial-event map the decoder mutates.
///
/// Every 500 ms it scans the map and gives up on any partial more than 32 slots
/// behind the ingester's latest-chain-slot counter.
///
/// Those entries are flushed to `solana.dead_letter` with a reason of
/// `AccountUpdateMissing` or `TxUpdateMissing` depending on which half was
/// missing.
pub(crate) struct PartialEventWatchdog {
    /// Persistence layer.
    pub persistence: Persistence,

    /// Shared in-memory map of partial events keyed by `(slot, signature)`.
    ///
    /// The decoder holds a clone of this `Arc` and both inserts and removes
    /// halves as it processes them.
    pub partials: Arc<DashMap<(Slot, Signature), PartialHalf>>,
}

impl PartialEventWatchdog {
    /// Construct a new watchdog.
    pub fn new(
        persistence: Persistence,
        partials: Arc<DashMap<(Slot, Signature), PartialHalf>>,
    ) -> Self {
        Self {
            persistence,
            partials,
        }
    }

    /// Outer loop. Runs the periodic scan over the shared partial-event map.
    pub async fn run(&mut self) -> Result<(), PersistenceError> {
        unimplemented!()
    }
}
