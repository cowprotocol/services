//! The partial-event watchdog.

// TODO: This file only declares the component skeleton. The `run` body is
// `unimplemented!`; the lag-detection and dead-letter logic arrive in a later
// change.

use {
    crate::{
        traits::store::Store,
        types::{
            errors::StoreError,
            shared::{PartialEvent, PartialEventKey},
        },
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
pub struct PartialEventWatchdog<St: Store> {
    /// Store implementor.
    pub store: St,

    /// Shared in-memory map of partial events keyed by `PartialEventKey`.
    ///
    /// The decoder holds a clone of this `Arc` and both inserts and removes
    /// halves as it processes them.
    pub partials: Arc<DashMap<PartialEventKey, PartialEvent>>,
}

impl<St: Store> PartialEventWatchdog<St> {
    /// Construct a new watchdog.
    pub fn new(store: St, partials: Arc<DashMap<PartialEventKey, PartialEvent>>) -> Self {
        Self { store, partials }
    }

    /// Outer loop. Runs the periodic scan over the shared partial-event map.
    pub async fn run(&mut self) -> Result<(), StoreError> {
        unimplemented!()
    }
}
