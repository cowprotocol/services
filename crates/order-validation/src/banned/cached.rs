//! Verdict cache in front of the Hermod client. A miss asks Hermod and stores
//! the answer. A background task checks the cached addresses again. If Hermod
//! gives no status for an address, its cached verdict stays.

use {
    super::{
        hermod::{Client as Hermod, Status},
        metrics::Metrics,
    },
    alloy_primitives::Address,
    moka::{policy::EvictionPolicy, sync::Cache},
    std::{
        collections::{HashMap, HashSet},
        sync::{
            Arc,
            Weak,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    },
};

/// `true` if banned. Shared so that a refresh can replace the verdict in
/// place: an `insert` would count as a request and keep the address cached.
type Verdict = Arc<AtomicBool>;

pub(super) struct Cached {
    hermod: Hermod,
    verdicts: Cache<Address, Verdict>,
}

impl Cached {
    /// Checks the cached addresses again every `refresh_interval`. An address
    /// that nobody requested for `max_idle` leaves the cache.
    pub(super) fn new(
        hermod: Hermod,
        max_entries: u64,
        refresh_interval: Duration,
        max_idle: Duration,
    ) -> Arc<Self> {
        assert!(
            !refresh_interval.is_zero(),
            "banned users refresh interval must not be zero"
        );
        let cached = Arc::new(Self {
            hermod,
            verdicts: verdict_cache(max_entries, max_idle),
        });
        cached.spawn_refresh_task(refresh_interval);
        cached
    }

    /// Returns the subset of `addresses` that is banned. Only the addresses
    /// without a cached verdict are sent to Hermod.
    pub(super) async fn banned(&self, addresses: &HashSet<Address>) -> HashSet<Address> {
        let mut banned = HashSet::new();
        let mut missing = HashSet::new();
        for address in addresses {
            match self.verdicts.get(address) {
                Some(verdict) if verdict.load(Ordering::Relaxed) => banned.insert(*address),
                Some(_) => false,
                None => missing.insert(*address),
            };
        }
        Metrics::cache(addresses.len() - missing.len(), missing.len());
        if missing.is_empty() {
            return banned;
        }

        for (address, status) in self.hermod.fetch_statuses(&missing).await {
            // An address with an unknown status has no cached verdict, so it
            // passes. It is cached as not banned so that the next requests do
            // not wait for Hermod; the next refresh that succeeds corrects it.
            let is_banned = status == Status::Banned;
            if is_banned {
                banned.insert(address);
            }
            self.verdicts
                .insert(address, Arc::new(AtomicBool::new(is_banned)));
        }
        banned
    }

    /// Checks all cached addresses again. Reads them with `iter`, which does
    /// not count as a request.
    async fn refresh(&self) {
        let cached: Vec<(Arc<Address>, Verdict)> = self.verdicts.iter().collect();
        let addresses = cached.iter().map(|(address, _)| **address).collect();
        let statuses = self.hermod.fetch_statuses(&addresses).await;
        store_verdicts(&cached, &statuses);
    }

    /// Holds a [`Weak`] handle so the task exits once the last external
    /// [`Arc`] is dropped.
    fn spawn_refresh_task(self: &Arc<Self>, refresh_interval: Duration) {
        let weak: Weak<Self> = Arc::downgrade(self);
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let Some(this) = weak.upgrade() else { return };
                this.refresh().await;
            }
        });
    }
}

/// Stores the verdicts of a refresh. An address with an unknown status keeps
/// its cached verdict: a failed request must not clear a ban.
fn store_verdicts(cached: &[(Arc<Address>, Verdict)], statuses: &HashMap<Address, Status>) {
    for (address, verdict) in cached {
        match statuses.get(&**address) {
            Some(Status::Banned) => verdict.store(true, Ordering::Relaxed),
            Some(Status::NotBanned) => verdict.store(false, Ordering::Relaxed),
            Some(Status::Unknown) | None => {}
        }
    }
}

/// LRU, because the default policy can refuse a new address when the cache is
/// full.
fn verdict_cache(max_entries: u64, max_idle: Duration) -> Cache<Address, Verdict> {
    Cache::builder()
        .max_capacity(max_entries)
        .eviction_policy(EvictionPolicy::lru())
        .time_to_idle(max_idle)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_status_keeps_the_cached_verdict() {
        let address = Address::repeat_byte;
        let entry = |byte, banned: bool| (Arc::new(address(byte)), Verdict::new(banned.into()));
        let cached = [
            entry(1, true),
            entry(2, false),
            entry(3, true),
            entry(4, false),
        ];
        // Hermod gave no verdict for 1 and 2, cleared 3 and banned 4.
        let statuses = HashMap::from([
            (address(1), Status::Unknown),
            (address(2), Status::Unknown),
            (address(3), Status::NotBanned),
            (address(4), Status::Banned),
        ]);

        store_verdicts(&cached, &statuses);

        let verdicts = cached.map(|(_, verdict)| verdict.load(Ordering::Relaxed));
        assert_eq!(verdicts, [true, false, false, true]);
    }

    #[test]
    fn full_cache_evicts_the_oldest_address() {
        let verdicts = verdict_cache(1, Duration::from_secs(60));
        let old = Address::repeat_byte(1);
        let new = Address::repeat_byte(2);

        verdicts.insert(old, Verdict::default());
        verdicts.insert(new, Verdict::default());
        verdicts.run_pending_tasks();

        assert!(verdicts.get(&old).is_none());
        assert!(verdicts.get(&new).is_some());
    }

    #[test]
    fn iterating_does_not_keep_an_address_cached() {
        let max_idle = Duration::from_millis(50);
        let verdicts = verdict_cache(10, max_idle);
        let address = Address::repeat_byte(1);
        verdicts.insert(address, Verdict::default());

        // No pause is longer than `max_idle`, so an iteration that counted
        // as a request would keep the address cached.
        for _ in 0..3 {
            std::thread::sleep(max_idle * 3 / 5);
            verdicts.iter().count();
        }
        verdicts.run_pending_tasks();

        assert!(verdicts.get(&address).is_none());
    }
}
