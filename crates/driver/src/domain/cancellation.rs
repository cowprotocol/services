//! Solver-initiated cancellation of an in-flight settlement.
//!
//! A solver only learns that it won an auction from the
//! [`settlement_started`](crate::infra::notify::settlement_started)
//! notification, which the driver emits once `/settle` is already being
//! processed. Everything a solver could decide before that point is therefore
//! out of reach, and the useful window runs from that notification until the
//! settlement is mined or the submission deadline passes.
//!
//! [`CancelSignal`] carries that decision across the checkpoints in that
//! window. It is deliberately one-way and idempotent: once raised it stays
//! raised, so a single request from the solver is observed by every checkpoint
//! still ahead of it rather than having to be repeated per stage.
//!
//! [`Registry`] connects the two sides: the API handler knows a solution by the
//! same `(auction, solution)` pair the autopilot used to request it, and the
//! registry turns that pair into the signal the submission in flight for it is
//! watching.

use {
    crate::domain::competition::auction,
    std::{
        collections::{HashMap, VecDeque},
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
    },
    tokio::sync::Notify,
};

/// A one-way, idempotent cancellation flag shared between the API handler that
/// raises it and the submission path that observes it.
#[derive(Clone, Debug, Default)]
pub struct CancelSignal(Arc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    raised: AtomicBool,
    /// Wakes waiters so the submission loop reacts when the request arrives
    /// instead of at the next block boundary. With a 5 block submission
    /// deadline a block is a fifth of the whole budget, so the difference is
    /// not academic.
    notify: Notify,
}

impl CancelSignal {
    /// Raise the flag. Returns whether this call was the one that raised it,
    /// so the caller can tell a first cancellation from a repeat.
    pub fn raise(&self) -> bool {
        let first = !self.0.raised.swap(true, Ordering::SeqCst);
        // Notify regardless: a waiter may have been registered between a
        // previous raise and now.
        self.0.notify.notify_waiters();
        first
    }

    pub fn is_raised(&self) -> bool {
        self.0.raised.load(Ordering::SeqCst)
    }

    /// Resolves once the flag is raised. Returns immediately if it already is.
    ///
    /// `Notify::notified()` only observes notifications sent after the future
    /// is created, so the flag is re-checked after registering to close the
    /// race with a concurrent [`Self::raise`].
    pub async fn raised(&self) {
        loop {
            if self.is_raised() {
                return;
            }
            let notified = self.0.notify.notified();
            if self.is_raised() {
                return;
            }
            notified.await;
        }
    }
}

/// Identifies a settlement across the `/settle` and `/cancel` requests that
/// refer to it. Both carry the same pair, which is all the solver knows about a
/// settlement it has handed over.
pub type SolutionKey = (auction::Id, u64);

/// How many solutions withdrawn ahead of their `/settle` are remembered.
/// Bounded because nothing guarantees that a `/settle` ever follows: the
/// autopilot may well have picked another solver. The gap between the two
/// requests is milliseconds, so a handful of slots covers the race, and the
/// oldest entries are the ones safest to forget.
const MAX_PENDING_WITHDRAWALS: usize = 32;

/// Routes a solver's withdrawal request to the submission in flight for that
/// solution.
#[derive(Debug, Default)]
pub struct Registry {
    in_flight: Mutex<HashMap<SolutionKey, CancelSignal>>,
    /// Solutions withdrawn before a submission registered itself, oldest
    /// first.
    pending: Mutex<VecDeque<SolutionKey>>,
}

/// What a withdrawal request found when it arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Withdrawal {
    /// A submission was in flight and has been told to stop.
    Signalled,
    /// The solution had already been withdrawn; this request changed nothing.
    AlreadyWithdrawn,
    /// No submission in flight yet. Recorded, so the submission is withdrawn
    /// the moment it starts.
    Recorded,
}

impl Registry {
    /// Register a submission that is about to start and hand back the signal
    /// its checkpoints observe, plus a [`Registration`] that keeps it
    /// reachable. The signal comes back already raised if the solver withdrew
    /// this solution before `/settle` arrived.
    pub fn register(self: &Arc<Self>, key: SolutionKey) -> (CancelSignal, Registration) {
        let signal = CancelSignal::default();
        if self.take_pending(key) {
            signal.raise();
        }
        self.in_flight.lock().unwrap().insert(key, signal.clone());
        (
            signal,
            Registration {
                registry: Arc::clone(self),
                key,
            },
        )
    }

    /// Withdraw a settlement on the solver's behalf.
    pub fn withdraw(&self, key: SolutionKey) -> Withdrawal {
        // Cloned out of the map so the lock is not held while raising.
        let in_flight = self.in_flight.lock().unwrap().get(&key).cloned();
        if let Some(signal) = in_flight {
            return match signal.raise() {
                true => Withdrawal::Signalled,
                false => Withdrawal::AlreadyWithdrawn,
            };
        }

        let mut pending = self.pending.lock().unwrap();
        if pending.contains(&key) {
            return Withdrawal::AlreadyWithdrawn;
        }
        while pending.len() >= MAX_PENDING_WITHDRAWALS {
            pending.pop_front();
        }
        pending.push_back(key);
        Withdrawal::Recorded
    }

    /// Consume a withdrawal recorded before the submission started, if any.
    fn take_pending(&self, key: SolutionKey) -> bool {
        let mut pending = self.pending.lock().unwrap();
        match pending.iter().position(|recorded| *recorded == key) {
            Some(index) => {
                pending.remove(index);
                true
            }
            None => false,
        }
    }
}

/// Keeps a submission reachable by [`Registry::withdraw`] for as long as it is
/// alive. Dropping it deregisters the submission, which also covers the settle
/// task being aborted because the caller disconnected.
#[derive(Debug)]
pub struct Registration {
    registry: Arc<Registry>,
    key: SolutionKey,
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.registry.in_flight.lock().unwrap().remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The solver sends one request; every checkpoint still ahead of it has to
    /// see the cancellation without the solver repeating itself.
    #[test]
    fn a_raised_signal_stays_raised() {
        let signal = CancelSignal::default();
        signal.raise();

        assert!(signal.is_raised());
    }

    /// The route reports back whether it was the first to cancel, so a retry
    /// does not read as a fresh cancellation.
    #[test]
    fn only_the_first_raise_reports_itself_as_first() {
        let signal = CancelSignal::default();
        signal.raise();

        assert!(!signal.raise());
    }

    /// Clones share one flag: the API handler and the submission loop hold
    /// separate handles to the same decision.
    #[test]
    fn a_clone_observes_a_raise_on_the_original() {
        let signal = CancelSignal::default();
        let clone = signal.clone();
        signal.raise();

        assert!(clone.is_raised());
    }

    /// Awaiting a signal that is already raised must not block, otherwise a
    /// cancellation that arrives before the checkpoint is reached would hang
    /// the submission loop instead of ending it.
    #[tokio::test]
    async fn awaiting_an_already_raised_signal_returns_immediately() {
        let signal = CancelSignal::default();
        signal.raise();

        signal.raised().await;
    }

    /// The loop waits on the signal before the request arrives; it has to wake
    /// on the raise rather than on the next block.
    #[tokio::test]
    async fn awaiting_wakes_on_a_later_raise() {
        let signal = CancelSignal::default();
        let waiter = signal.clone();
        let handle = tokio::spawn(async move { waiter.raised().await });

        tokio::task::yield_now().await;
        signal.raise();

        handle.await.unwrap();
    }
    const KEY: SolutionKey = (auction::Id(1), 7);

    /// The whole point of the registry: a request naming a solution reaches
    /// the submission that is in flight for it.
    #[test]
    fn withdrawing_an_in_flight_settlement_raises_its_signal() {
        let registry = Arc::new(Registry::default());
        let (signal, _registration) = registry.register(KEY);

        assert_eq!(registry.withdraw(KEY), Withdrawal::Signalled);
        assert!(signal.is_raised());
    }

    /// `/cancel` can beat the autopilot's `/settle` by a few milliseconds. The
    /// withdrawal has to survive that gap, otherwise it is silently ignored
    /// and the settlement goes out anyway.
    #[test]
    fn a_withdrawal_recorded_before_the_submission_starts_is_applied_on_registration() {
        let registry = Arc::new(Registry::default());

        assert_eq!(registry.withdraw(KEY), Withdrawal::Recorded);
        let (signal, _registration) = registry.register(KEY);

        assert!(signal.is_raised());
    }

    /// A recorded withdrawal is consumed by the submission it was meant for,
    /// so a later retry of the same solution is not cancelled by a stale
    /// record.
    #[test]
    fn a_recorded_withdrawal_only_applies_to_the_next_submission() {
        let registry = Arc::new(Registry::default());
        registry.withdraw(KEY);
        drop(registry.register(KEY));

        let (signal, _registration) = registry.register(KEY);

        assert!(!signal.is_raised());
    }

    /// Once submission has finished there is nothing left to withdraw, and the
    /// registry must not keep the entry alive.
    #[test]
    fn a_finished_submission_is_no_longer_reachable() {
        let registry = Arc::new(Registry::default());
        drop(registry.register(KEY));

        assert_eq!(registry.withdraw(KEY), Withdrawal::Recorded);
    }

    /// Withdrawals are per solution: cancelling one settlement must not take
    /// down another that happens to be in flight at the same time.
    #[test]
    fn a_withdrawal_leaves_other_solutions_alone() {
        let registry = Arc::new(Registry::default());
        let (signal, _registration) = registry.register(KEY);
        let (other, _other_registration) = registry.register((auction::Id(1), 8));

        registry.withdraw(KEY);

        assert!(signal.is_raised());
        assert!(!other.is_raised());
    }

    /// A retried request reports that the solution was already withdrawn
    /// rather than claiming a second cancellation.
    #[test]
    fn a_repeated_withdrawal_reports_itself_as_already_withdrawn() {
        let registry = Arc::new(Registry::default());
        let (_signal, _registration) = registry.register(KEY);
        registry.withdraw(KEY);

        assert_eq!(registry.withdraw(KEY), Withdrawal::AlreadyWithdrawn);
    }

    /// Same, for a withdrawal that has only been recorded: repeating it must
    /// not grow the bounded set.
    #[test]
    fn a_repeated_recorded_withdrawal_is_not_recorded_twice() {
        let registry = Arc::new(Registry::default());
        registry.withdraw(KEY);

        assert_eq!(registry.withdraw(KEY), Withdrawal::AlreadyWithdrawn);
        assert_eq!(registry.pending.lock().unwrap().len(), 1);
    }

    /// Nothing guarantees a `/settle` for a recorded withdrawal, so the set
    /// has to stay bounded instead of growing for the driver's lifetime.
    #[test]
    fn recorded_withdrawals_are_bounded_and_evict_the_oldest() {
        let registry = Arc::new(Registry::default());
        for solution_id in 0..u64::try_from(MAX_PENDING_WITHDRAWALS).unwrap() + 1 {
            registry.withdraw((auction::Id(1), solution_id));
        }

        assert_eq!(
            registry.pending.lock().unwrap().len(),
            MAX_PENDING_WITHDRAWALS
        );
        // The oldest was evicted, the newest is still remembered.
        let (evicted, _registration) = registry.register((auction::Id(1), 0));
        let (kept, _registration) = registry.register((
            auction::Id(1),
            u64::try_from(MAX_PENDING_WITHDRAWALS).unwrap(),
        ));
        assert!(!evicted.is_raised());
        assert!(kept.is_raised());
    }
}
