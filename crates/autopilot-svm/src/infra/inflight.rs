//! In-memory hold-out of orders with a settlement in flight.
//!
//! An order dispatched for settlement must not re-enter the next auction
//! while the first settlement can still land: a second winner would
//! double-settle it. The driver stops waiting at the submission deadline,
//! but the transaction it sent stays landable until its blockhash expires,
//! up to `MAX_PROCESSING_AGE` slots later, so held orders expire only at
//! the deadline plus that lifetime. Two events end a hold early: the
//! settlement is observed on chain (the auction's orders are done), or the
//! driver rejects provably before any send. Everything else, a submit error
//! or an exceeded deadline, may have left the transaction on the wire and
//! keeps the hold.
//!
//! The map lives in memory: a restart forgets it and reopens the window
//! until the entries would have expired.

use {
    chain_types::solana::{IntentHash, Pubkey},
    solana_sdk::clock::MAX_PROCESSING_AGE,
    std::{
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex},
    },
};

#[derive(Default)]
struct State {
    /// Held order uids and the slot their hold expires at.
    held: HashMap<IntentHash, u64>,
    /// The orders of each dispatched settlement, keyed the way the indexer
    /// identifies a landed settlement, with the dispatch's expiry slot.
    settlements: HashMap<(i64, Pubkey), (Vec<IntentHash>, u64)>,
}

/// Order uids held out of auction cuts until their settlement transaction
/// cannot land any more.
#[derive(Clone, Default)]
pub struct InFlightOrders(Arc<Mutex<State>>);

impl InFlightOrders {
    /// Hold the settlement's orders until the deadline slot plus the
    /// blockhash lifetime, the last slot its transaction could still land.
    /// An order already held keeps the later expiry.
    pub fn hold(&self, auction_id: i64, solver: Pubkey, uids: Vec<IntentHash>, deadline_slot: u64) {
        let expiry = deadline_slot.saturating_add(MAX_PROCESSING_AGE as u64);
        let mut state = self.0.lock().expect("mutex poisoned");
        for uid in &uids {
            state
                .held
                .entry(*uid)
                .and_modify(|held_until| *held_until = (*held_until).max(expiry))
                .or_insert(expiry);
        }
        state
            .settlements
            .insert((auction_id, solver), (uids, expiry));
    }

    /// Release the orders: their settlement provably never went out, so no
    /// second settlement can collide.
    pub fn release(&self, uids: impl IntoIterator<Item = IntentHash>) {
        let mut state = self.0.lock().expect("mutex poisoned");
        for uid in uids {
            state.held.remove(&uid);
        }
    }

    /// Release the orders of a settlement observed on chain. The auction is
    /// settled for this solver, nothing else can execute these orders.
    pub fn release_landed(&self, auction_id: i64, solver: Pubkey) {
        let mut state = self.0.lock().expect("mutex poisoned");
        if let Some((uids, _)) = state.settlements.remove(&(auction_id, solver)) {
            for uid in uids {
                state.held.remove(&uid);
            }
        }
    }

    /// The orders still held at the tip. Expired entries are pruned on the
    /// way.
    pub fn held_at(&self, tip: u64) -> HashSet<IntentHash> {
        let mut state = self.0.lock().expect("mutex poisoned");
        state.held.retain(|_, held_until| *held_until >= tip);
        state.settlements.retain(|_, (_, expiry)| *expiry >= tip);
        state.held.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOLVER: Pubkey = Pubkey([9; 32]);

    #[test]
    fn holds_through_the_blockhash_lifetime_past_the_deadline() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        inflight.hold(1, SOLVER, vec![uid], 100);
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
        inflight.hold(1, SOLVER, vec![uid, other], 100);
        inflight.release([uid]);

        let held = inflight.held_at(50);
        assert!(!held.contains(&uid));
        assert!(held.contains(&other));
    }

    #[test]
    fn a_landed_settlement_releases_its_orders() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        let unrelated = IntentHash([8; 32]);
        inflight.hold(1, SOLVER, vec![uid], 100);
        inflight.hold(2, SOLVER, vec![unrelated], 100);

        inflight.release_landed(1, SOLVER);
        let held = inflight.held_at(50);
        assert!(!held.contains(&uid));
        assert!(held.contains(&unrelated));

        // A landing for a solver without a dispatch is a no-op.
        inflight.release_landed(3, Pubkey([1; 32]));
        assert!(inflight.held_at(50).contains(&unrelated));
    }

    #[test]
    fn a_second_dispatch_keeps_the_later_expiry() {
        let inflight = InFlightOrders::default();
        let uid = IntentHash([7; 32]);
        inflight.hold(1, SOLVER, vec![uid], 100);
        inflight.hold(2, SOLVER, vec![uid], 90);

        assert!(
            inflight
                .held_at(100 + MAX_PROCESSING_AGE as u64)
                .contains(&uid)
        );
    }
}
