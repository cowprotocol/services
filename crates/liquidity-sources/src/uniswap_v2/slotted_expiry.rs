//! Expiry policy for the negative pool cache.

use {
    model::TokenPair,
    moka::Expiry,
    std::{
        hash::{BuildHasher, RandomState},
        time::{Duration, Instant},
    },
};

/// Assigns every token pair a fixed slot in the probe cycle, derived from the
/// pair, and expires its entry at the next occurrence of that slot.
///
/// The point is to spread the pairs uniformly across the cycle. Without this,
/// every suppressed pair expires in the same auction, and on mainnet that is a
/// 5x spike in `eth_call` rate once an hour. Because the slot is absolute
/// rather than measured from the insert, a pair settles onto an exact `base`
/// cadence, so spreading the probes costs no extra probes.
///
/// The first window after a pair is discovered is the exception: it runs only
/// until the next slot, so it is uniform in `(0, base]` rather than a full
/// `base`. A pair can therefore be probed twice in quick succession when it
/// first appears, or reappears after dropping out of the auction. `base` is
/// always an upper bound.
///
/// The slot is derived rather than drawn because an expired entry is removed:
/// a re-probe is a fresh insert with no memory of the previous deadline, so
/// there is nowhere to keep a phase. A drawn value would have to be redrawn on
/// every insert, which pulls the mean cadence below `base`. Deriving it also
/// makes the policy deterministic, and makes it irrelevant whether moka treats
/// a re-probe as a create or as an update.
pub struct SlottedExpiry {
    /// Cycle length in nanoseconds. Clamped so the arithmetic below stays in
    /// `u64`; the clamp is ~584 years and no configuration comes near it.
    cycle: u64,
    epoch: Instant,
    /// Seeded per process, so the slot a pair lands on is not predictable from
    /// the pair alone and does not correlate between venues.
    hasher: RandomState,
}

impl SlottedExpiry {
    pub fn new(base: Duration) -> Self {
        Self {
            cycle: u64::try_from(base.as_nanos()).unwrap_or(u64::MAX),
            epoch: Instant::now(),
            hasher: RandomState::new(),
        }
    }

    fn lifespan(&self, pair: &TokenPair) -> Option<Duration> {
        self.lifespan_at(pair, self.epoch.elapsed())
    }

    /// Time from `elapsed` until this pair's next slot.
    fn lifespan_at(&self, pair: &TokenPair, elapsed: Duration) -> Option<Duration> {
        // A zero cycle is configurable (`missing-pool-cache-time = 0s`) and
        // disables suppression. It also has to be caught before the modulo.
        if self.cycle == 0 {
            return Some(Duration::ZERO);
        }

        let cycle = u128::from(self.cycle);
        let slot = self.hasher.hash_one(pair) % self.cycle;
        let now = u64::try_from(elapsed.as_nanos() % cycle).unwrap_or(0);

        // Both branches stay below `cycle`, so neither can overflow, and
        // landing exactly on the slot waits a whole cycle rather than expiring
        // the entry immediately.
        let wait = match slot.checked_sub(now) {
            Some(0) | None => self.cycle - (now - slot),
            Some(wait) => wait,
        };

        Some(Duration::from_nanos(wait))
    }
}

impl Expiry<TokenPair, ()> for SlottedExpiry {
    fn expire_after_create(&self, pair: &TokenPair, _: &(), _: Instant) -> Option<Duration> {
        self.lifespan(pair)
    }

    /// Overriding this is load-bearing. The default echoes the remaining
    /// duration back, and for an already-past deadline moka then skips
    /// rescheduling entirely, leaving the entry permanently expired and
    /// re-probed on every single fetch.
    fn expire_after_update(
        &self,
        pair: &TokenPair,
        _: &(),
        _: Instant,
        _: Option<Duration>,
    ) -> Option<Duration> {
        self.lifespan(pair)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::Address};

    /// The slot is a fixed point in the cycle, so the wait shrinks towards it
    /// and then rolls over to a full cycle. Driving `lifespan_at` directly
    /// keeps this independent of the process-random slot the pair happens
    /// to get.
    #[test]
    fn a_pair_keeps_its_slot_as_time_passes() {
        let base = Duration::from_secs(3600);
        let expiry = SlottedExpiry::new(base);
        let pair = TokenPair::new(Address::with_last_byte(1), Address::with_last_byte(2)).unwrap();

        let to_slot = expiry.lifespan_at(&pair, Duration::ZERO).unwrap();
        assert!(to_slot > Duration::ZERO && to_slot <= base);

        assert_eq!(
            expiry
                .lifespan_at(&pair, to_slot - Duration::from_nanos(1))
                .unwrap(),
            Duration::from_nanos(1),
            "the deadline moved instead of staying put"
        );
        assert_eq!(
            expiry.lifespan_at(&pair, to_slot).unwrap(),
            base,
            "landing on the slot must wait a whole cycle, not expire at once"
        );
    }

    /// Guards the `expire_after_update` override. Without it the hook echoes
    /// the inherited duration back rather than returning to the pair's
    /// slot.
    #[test]
    fn re_probing_returns_to_the_pairs_slot() {
        let expiry = SlottedExpiry::new(Duration::from_secs(3600));
        let pair = TokenPair::new(Address::with_last_byte(1), Address::with_last_byte(2)).unwrap();
        let now = Instant::now();

        let on_create = expiry.expire_after_create(&pair, &(), now).unwrap();
        let on_update = expiry
            .expire_after_update(&pair, &(), now, Some(Duration::from_secs(1)))
            .unwrap();

        assert!(
            on_create.abs_diff(on_update) < Duration::from_millis(10),
            "update returned {on_update:?} instead of the slot at {on_create:?}"
        );
    }
}
