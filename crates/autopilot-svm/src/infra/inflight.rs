//! In-memory hold-out of orders with a settlement in flight.
//!
//! An order dispatched for settlement must not re-enter the next auction
//! while the first settlement can still land: a second winner would
//! double-settle it. The driver stops waiting at the submission deadline,
//! but the transaction it sent stays landable until its blockhash expires,
//! up to `MAX_PROCESSING_AGE` slots later, so held orders expire only at
//! the deadline plus that lifetime. The driver also reports failures it
//! cannot prove (a send error may have reached the network), so held orders
//! are never released early.
//!
//! The map lives in memory: a restart forgets it and reopens the window
//! until the entries would have expired.

use {
    chain_types::solana::IntentHash,
    solana_sdk::clock::MAX_PROCESSING_AGE,
    std::{
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex},
    },
};

/// Order uids held out of auction cuts until their settlement transaction
/// cannot land any more.
#[derive(Clone, Default)]
pub struct InFlightOrders(Arc<Mutex<HashMap<IntentHash, u64>>>);

impl InFlightOrders {
    /// Hold the orders until the deadline slot plus the blockhash lifetime,
    /// the last slot the settlement transaction could still land. An order
    /// already held keeps the later expiry.
    pub fn hold(&self, uids: impl IntoIterator<Item = IntentHash>, deadline_slot: u64) {
        let expiry = deadline_slot.saturating_add(MAX_PROCESSING_AGE as u64);
        let mut held = self.0.lock().expect("mutex poisoned");
        for uid in uids {
            held.entry(uid)
                .and_modify(|held_until| *held_until = (*held_until).max(expiry))
                .or_insert(expiry);
        }
    }

    /// Release the orders: their settlement landed or provably never went
    /// out, so no second settlement can collide.
    pub fn release(&self, uids: impl IntoIterator<Item = IntentHash>) {
        let mut held = self.0.lock().expect("mutex poisoned");
        for uid in uids {
            held.remove(&uid);
        }
    }

    /// The orders still held at the tip. Expired entries are pruned on the
    /// way.
    pub fn held_at(&self, tip: u64) -> HashSet<IntentHash> {
        let mut held = self.0.lock().expect("mutex poisoned");
        held.retain(|_, held_until| *held_until >= tip);
        held.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_through_the_blockhash_lifetime_past_the_deadline() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        inflight.hold([uid], 100);
        let expiry = 100 + MAX_PROCESSING_AGE as u64;

        assert!(inflight.held_at(100).contains(&uid));
        assert!(inflight.held_at(expiry).contains(&uid));
        assert!(inflight.held_at(expiry + 1).is_empty());
    }

    #[test]
    fn released_orders_re_enter_immediately() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        let other = IntentHash([8; 32]);
        inflight.hold([uid, other], 100);
        inflight.release([uid]);

        let held = inflight.held_at(50);
        assert!(!held.contains(&uid));
        assert!(held.contains(&other));
    }

    #[test]
    fn a_second_dispatch_keeps_the_later_expiry() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        inflight.hold([uid], 100);
        inflight.hold([uid], 90);

        assert!(
            inflight
                .held_at(100 + MAX_PROCESSING_AGE as u64)
                .contains(&uid)
        );
    }
}
