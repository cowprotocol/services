use {
    crate::{
        domain::competition::{order::Uid, risk_detector::Quality},
        infra::{observe::metrics, solver},
    },
    dashmap::DashMap,
    model::time::now_in_epoch_seconds,
    std::{
        sync::{Arc, Weak},
        time::{Duration, Instant},
    },
};

#[derive(Debug)]
struct OrderStatistics {
    attempts: u32,
    fails: u32,
    flagged_unsupported_at: Option<Instant>,
    /// When an order was last seen in a solution. This
    /// timestamp is used to determine whether the order's
    /// metrics can be evicted from the cache to avoid bloat.
    last_seen_at: Instant,
}

/// Monitors orders to determine whether they are considered "unsupported" based
/// on the ratio of failing to total settlement encoding attempts. An order must
/// have participated in at least `REQUIRED_MEASUREMENTS` attempts to be
/// evaluated. If, at that point, the ratio of failures is greater than or equal
/// to `FAILURE_RATIO`, the order is considered unsupported.
#[derive(Clone)]
pub struct Detector {
    failure_ratio: f64,
    required_measurements: u32,
    counter: Arc<DashMap<Uid, OrderStatistics>>,
    log_only: bool,
    order_freeze_time: Duration,
    solver: solver::Name,
}

impl Detector {
    pub fn new(
        failure_ratio: f64,
        required_measurements: u32,
        log_only: bool,
        order_freeze_time: Duration,
        gc_interval: Duration,
        gc_max_age: Duration,
        solver: solver::Name,
    ) -> Self {
        let counter = Arc::new(DashMap::default());

        Self::spawn_gc_task(Arc::downgrade(&counter), gc_interval, gc_max_age);

        Self {
            failure_ratio,
            required_measurements,
            counter: counter.clone(),
            log_only,
            order_freeze_time,
            solver,
        }
    }

    pub fn get_quality(&self, order: &Uid, now: Instant) -> Quality {
        let Some(stats) = self.counter.get(order) else {
            return Quality::Unknown;
        };

        if stats
            .flagged_unsupported_at
            .is_some_and(|t| now.duration_since(t) > self.order_freeze_time)
        {
            // Sometimes tokens only cause issues temporarily. If the token's freeze
            // period expired we pretend we don't have enough information to give it
            // another chance. If it still behaves badly it will get frozen immediately.
            return Quality::Unknown;
        }

        match self.log_only {
            true => Quality::Supported,
            false => self.quality_based_on_stats(&stats),
        }
    }

    fn quality_based_on_stats(&self, stats: &OrderStatistics) -> Quality {
        if stats.attempts < self.required_measurements {
            return Quality::Unknown;
        }
        let token_failure_ratio = f64::from(stats.fails) / f64::from(stats.attempts);
        match token_failure_ratio >= self.failure_ratio {
            true => Quality::Unsupported,
            false => Quality::Supported,
        }
    }

    /// Updates the orders that participated in settlements by
    /// incrementing their attempt count.
    /// `failure` indicates whether the settlement was successful or not.
    pub fn update_orders(&self, orders: &[Uid], failure: bool) {
        let now = Instant::now();
        let mut new_unsupported_orders = vec![];
        orders.iter().for_each(|order| {
            let mut stats = self
                .counter
                .entry(*order)
                .and_modify(|counter| {
                    counter.attempts += 1;
                    counter.fails += u32::from(failure);
                    counter.last_seen_at = now;
                })
                .or_insert_with(|| OrderStatistics {
                    attempts: 1,
                    fails: u32::from(failure),
                    flagged_unsupported_at: None,
                    last_seen_at: now,
                });

            // order needs to be frozen as unsupported for a while
            if self.quality_based_on_stats(&stats) == Quality::Unsupported
                && stats
                    .flagged_unsupported_at
                    .is_none_or(|t| now.duration_since(t) > self.order_freeze_time)
            {
                new_unsupported_orders.push(order);
                stats.flagged_unsupported_at = Some(now);
            }
        });

        if !new_unsupported_orders.is_empty() {
            tracing::debug!(
                orders = ?new_unsupported_orders,
                "mark order as unsupported"
            );
            metrics::get()
                .bad_orders_detected
                .with_label_values(&[&self.solver.0])
                .inc_by(new_unsupported_orders.len() as u64);
        }
    }

    /// Spawns a background tasks that periodically evicts items from the cache
    /// that are no longer relevant to avoid bloat.
    fn spawn_gc_task(
        cache: Weak<DashMap<Uid, OrderStatistics>>,
        interval: Duration,
        max_age: Duration,
    ) {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            while let Some(cache) = cache.upgrade() {
                let now = Instant::now();
                let now_as_unix = now_in_epoch_seconds();

                cache.retain(|uid, stats| {
                    uid.valid_to() > now_as_unix && now.duration_since(stats.last_seen_at) < max_age
                });
                interval.tick().await;
            }
            tracing::debug!("terminating gc task because cache was dropped");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that an order only gets marked temporarily as unsupported.
    /// After the freeze period it will be allowed again.
    #[tokio::test]
    async fn unfreeze_bad_orders() {
        const FREEZE_DURATION: Duration = Duration::from_millis(50);
        let detector = Detector::new(
            0.5,
            2,
            false,
            FREEZE_DURATION,
            Duration::from_hours(1),
            Duration::from_hours(1),
            solver::Name("mysolver".to_string()),
        );

        let order = Uid::from_parts(Default::default(), Default::default(), u32::MAX);
        let order_quality = || detector.get_quality(&order, Instant::now());

        // order is reported as unknown while we don't have enough measurements
        assert_eq!(order_quality(), Quality::Unknown);
        detector.update_orders(&[order], true);
        assert_eq!(order_quality(), Quality::Unknown);
        detector.update_orders(&[order], true);

        // after we got enough measurements the order gets marked as bad
        assert_eq!(order_quality(), Quality::Unsupported);

        // after the freeze period is over the token gets reported as unknown again
        tokio::time::sleep(FREEZE_DURATION).await;
        assert_eq!(order_quality(), Quality::Unknown);

        // after an unfreeze another bad measurement is enough to freeze it again
        detector.update_orders(&[order], true);
        assert_eq!(order_quality(), Quality::Unsupported);
    }

    /// Tests that the GC task correctly evicts orders that are expired
    /// or have not been seen for the configured amount of time.
    #[tokio::test]
    async fn evict_outdated_entries() {
        const FREEZE_DURATION: Duration = Duration::from_millis(50);
        const GC_INTERVAL: Duration = Duration::from_millis(10);
        const GC_CYCLES_UNTIL_EVICTION: u32 = 5;
        let gc_max_age = GC_INTERVAL * GC_CYCLES_UNTIL_EVICTION;

        // this spawns a gc task that evicts entries from the cache
        let detector = Detector::new(
            0.5,
            2,
            false,
            FREEZE_DURATION,
            GC_INTERVAL,
            gc_max_age,
            solver::Name("mysolver".to_string()),
        );

        let long_valid_to = now_in_epoch_seconds() + 1000;
        let short_valid_to = 0; // already expired -> evict on first GC run

        let long_order = Uid::from_parts(Default::default(), Default::default(), long_valid_to);
        let short_order = Uid::from_parts(Default::default(), Default::default(), short_valid_to);

        assert_eq!(detector.counter.len(), 0);
        detector.update_orders(&[long_order, short_order], true);
        assert_eq!(detector.counter.len(), 2);
        assert!(detector.counter.get(&long_order).is_some());
        assert!(detector.counter.get(&short_order).is_some());

        // The gc task and this test operate on an interval. In order to avoid
        // issues due to variance we wait half a GC interval to make sure
        // our assertions always happen in the middle between 2 GC runs.
        tokio::time::sleep(GC_INTERVAL / 2).await;
        let mut interval = tokio::time::interval(GC_INTERVAL);

        // after 1 GC cycle the expired order was evicted
        assert_eq!(detector.counter.len(), 1);
        assert!(detector.counter.get(&long_order).is_some());

        for _ in 0..(GC_CYCLES_UNTIL_EVICTION - 1) {
            interval.tick().await;
        }

        // order was still not evicted because the max age has not been reached yet
        assert_eq!(detector.counter.len(), 1);
        assert!(detector.counter.get(&long_order).is_some());

        // add another measurement to extend lifetime in cache
        detector.update_orders(&[long_order], true);

        // metrics are still in the cache after almost max_age * 2
        for _ in 0..=(GC_CYCLES_UNTIL_EVICTION - 1) {
            interval.tick().await;
        }
        assert_eq!(detector.counter.len(), 1);
        assert!(detector.counter.get(&long_order).is_some());

        // after one more GC cycle the order finally gets evicted
        interval.tick().await;
        assert_eq!(detector.counter.len(), 0);
    }
}
