//! In-memory hold-out of orders with a settlement in flight.
//!
//! An order dispatched for settlement must not re-enter the next auction
//! before its submission deadline passes: the driver may still land the
//! first settlement, and a second winner would double-settle it. The driver
//! reports failures it cannot prove (a send error may have reached the
//! network), so held orders are never released early, they expire with the
//! deadline slot.
//!
//! The map lives in memory: a restart forgets it and reopens the window
//! until the deadline passes.

use {
    chain_types::solana::IntentHash,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
};

/// Order uids held out of auction cuts until their deadline slot passes.
#[derive(Clone, Default)]
pub struct InFlightOrders(Arc<Mutex<HashMap<IntentHash, u64>>>);

impl InFlightOrders {
    /// Hold the orders until the deadline slot. An order already held keeps
    /// the later deadline.
    pub fn hold(&self, uids: impl IntoIterator<Item = IntentHash>, deadline_slot: u64) {
        let mut held = self.0.lock().expect("mutex poisoned");
        for uid in uids {
            held.entry(uid)
                .and_modify(|deadline| *deadline = (*deadline).max(deadline_slot))
                .or_insert(deadline_slot);
        }
    }

    /// Whether the order is still held at the tip. Expired entries are
    /// pruned on the way.
    pub fn held(&self, uid: &IntentHash, tip: u64) -> bool {
        let mut held = self.0.lock().expect("mutex poisoned");
        held.retain(|_, deadline| *deadline >= tip);
        held.contains_key(uid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_until_the_deadline_slot() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        inflight.hold([uid], 100);

        assert!(inflight.held(&uid, 99));
        assert!(inflight.held(&uid, 100));
        assert!(!inflight.held(&uid, 101));
        assert!(!inflight.held(&IntentHash([8; 32]), 50));
    }

    #[test]
    fn a_second_dispatch_keeps_the_later_deadline() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        inflight.hold([uid], 100);
        inflight.hold([uid], 90);

        assert!(inflight.held(&uid, 100));
    }
}
