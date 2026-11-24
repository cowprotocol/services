//! Verdict cache in front of the Hermod client. A miss asks Hermod and stores
//! the answer. A background task checks the cached addresses again.

use {
    super::{hermod::Client as Hermod, metrics::Metrics},
    alloy_primitives::Address,
    moka::{policy::EvictionPolicy, sync::Cache},
    std::{
        collections::HashSet,
        sync::{
            Arc,
            Weak,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    },
};

/// How often the cached addresses are checked again.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
/// An address that nobody requested for this long leaves the cache.
const MAX_IDLE: Duration = Duration::from_secs(60);

/// `true` if banned. Shared so that a refresh can replace the verdict in
/// place: an `insert` would count as a request and keep the address cached.
type Verdict = Arc<AtomicBool>;

pub(super) struct Cached {
    hermod: Hermod,
    verdicts: Cache<Address, Verdict>,
}

impl Cached {
    pub(super) fn new(hermod: Hermod, max_entries: u64) -> Arc<Self> {
        let cached = Arc::new(Self {
            hermod,
            verdicts: verdict_cache(max_entries, MAX_IDLE),
        });
        cached.spawn_refresh_task();
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

        let newly_banned = self.hermod.banned(&missing).await;
        for address in missing {
            let verdict = AtomicBool::new(newly_banned.contains(&address));
            self.verdicts.insert(address, Arc::new(verdict));
        }
        banned.extend(newly_banned);
        banned
    }

    /// Checks all cached addresses again. Reads them with `iter`, which does
    /// not count as a request.
    async fn refresh(&self) {
        let cached: Vec<(Arc<Address>, Verdict)> = self.verdicts.iter().collect();
        let addresses = cached.iter().map(|(address, _)| **address).collect();
        let banned = self.hermod.banned(&addresses).await;
        for (address, verdict) in cached {
            verdict.store(banned.contains(&*address), Ordering::Relaxed);
        }
    }

    /// Holds a [`Weak`] handle so the task exits once the last external
    /// [`Arc`] is dropped.
    fn spawn_refresh_task(self: &Arc<Self>) {
        let weak: Weak<Self> = Arc::downgrade(self);
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(REFRESH_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let Some(this) = weak.upgrade() else { return };
                this.refresh().await;
            }
        });
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
    fn full_cache_evicts_the_oldest_address() {
        let verdicts = verdict_cache(1, MAX_IDLE);
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
