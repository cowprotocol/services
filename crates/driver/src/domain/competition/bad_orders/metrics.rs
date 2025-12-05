use {
    super::Quality,
    crate::{
        domain::competition::order,
        infra::{observe::metrics, solver},
    },
    dashmap::DashMap,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

#[derive(Default, Debug)]
struct OrderStatistics {
    attempts: u32,
    fails: u32,
    flagged_unsupported_at: Option<Instant>,
}

/// Monitors orders to determine whether they are considered "unsupported" based
/// on the ratio of failing to total settlement encoding attempts. An order must
/// have participated in at least `REQUIRED_MEASUREMENTS` settlement attempts to
/// be evaluated. If, at that point, the ratio of failures is greater than or
/// equal to `FAILURE_RATIO`, the order is considered unsupported.
///
/// This detector tracks settlement simulation failures at the order level
/// rather than the token level, avoiding the problem of banning good tokens due
/// to solver-specific issues or bad solutions.
#[derive(Clone)]
pub struct Detector {
    failure_ratio: f64,
    required_measurements: u32,
    counter: Arc<DashMap<order::Uid, OrderStatistics>>,
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
        solver: solver::Name,
    ) -> Self {
        Self {
            failure_ratio,
            required_measurements,
            counter: Default::default(),
            log_only,
            order_freeze_time,
            solver,
        }
    }

    pub fn get_quality(&self, uid: &order::Uid, now: Instant) -> Quality {
        let Some(stats) = self.counter.get(uid) else {
            return Quality::Unknown;
        };

        if stats
            .flagged_unsupported_at
            .is_some_and(|t| now.duration_since(t) > self.order_freeze_time)
        {
            // Sometimes orders only cause issues temporarily (e.g., insufficient balance
            // that gets topped up later). If the order's freeze period expired we pretend
            // we don't have enough information to give it another chance. If it still
            // behaves badly it will get frozen immediately.
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
        let order_failure_ratio = f64::from(stats.fails) / f64::from(stats.attempts);
        match order_failure_ratio >= self.failure_ratio {
            true => Quality::Unsupported,
            false => Quality::Supported,
        }
    }

    /// Updates the orders that participated in settlements by
    /// incrementing their attempt count.
    /// `failure` indicates whether the settlement encoding/simulation was
    /// successful or not.
    pub fn update_orders(&self, order_uids: &[order::Uid], failure: bool) {
        let now = Instant::now();
        let mut new_unsupported_orders = vec![];

        for uid in order_uids {
            let mut stats = self
                .counter
                .entry(*uid)
                .and_modify(|counter| {
                    counter.attempts += 1;
                    counter.fails += u32::from(failure);
                })
                .or_insert_with(|| OrderStatistics {
                    attempts: 1,
                    fails: u32::from(failure),
                    flagged_unsupported_at: None,
                });

            // order needs to be frozen as unsupported for a while
            if self.quality_based_on_stats(&stats) == Quality::Unsupported
                && stats
                    .flagged_unsupported_at
                    .is_none_or(|t| now.duration_since(t) > self.order_freeze_time)
            {
                new_unsupported_orders.push(*uid);
                stats.flagged_unsupported_at = Some(now);
            }
        }

        if !new_unsupported_orders.is_empty() {
            tracing::debug!(
                orders = ?new_unsupported_orders,
                "mark orders as unsupported"
            );
            metrics::get()
                .bad_orders_detected
                .with_label_values(&[&self.solver.0, "metrics"])
                .inc_by(new_unsupported_orders.len() as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::util::Bytes};

    fn test_uid(value: u8) -> order::Uid {
        order::Uid(Bytes([value; 56]))
    }

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
            solver::Name("mysolver".to_string()),
        );

        let order_a = test_uid(1);
        let order_b = test_uid(2);
        let order_quality = || detector.get_quality(&order_a, Instant::now());

        // order is reported as unknown while we don't have enough measurements
        assert_eq!(order_quality(), Quality::Unknown);
        detector.update_orders(&[order_a, order_b], true);
        assert_eq!(order_quality(), Quality::Unknown);
        detector.update_orders(&[order_a, order_b], true);

        // after we got enough measurements the order gets marked as bad
        assert_eq!(order_quality(), Quality::Unsupported);

        // after the freeze period is over the order gets reported as unknown again
        tokio::time::sleep(FREEZE_DURATION).await;
        assert_eq!(order_quality(), Quality::Unknown);

        // after an unfreeze another bad measurement is enough to freeze it again
        detector.update_orders(&[order_a, order_b], true);
        assert_eq!(order_quality(), Quality::Unsupported);
    }

    #[test]
    fn different_orders_tracked_independently() {
        let detector = Detector::new(
            0.5,
            2,
            false,
            Duration::from_secs(60),
            solver::Name("mysolver".to_string()),
        );

        let order_a = test_uid(1);
        let order_b = test_uid(2);

        // order_a fails twice
        detector.update_orders(&[order_a], true);
        detector.update_orders(&[order_a], true);

        // order_b succeeds twice
        detector.update_orders(&[order_b], false);
        detector.update_orders(&[order_b], false);

        let now = Instant::now();
        assert_eq!(detector.get_quality(&order_a, now), Quality::Unsupported);
        assert_eq!(detector.get_quality(&order_b, now), Quality::Supported);
    }
}
