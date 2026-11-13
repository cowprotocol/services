//! Canonical batches are auctions that start at fixed times. Orders placed
//! with `enableWaitForCow` only trade in canonical batches, so that orders
//! waiting for the same batch get a chance to be matched with each other.
//!
//! The autopilot decides which auctions are canonical batches and publishes
//! its [`Schedule`]. The orderbook reads the published schedule to validate
//! wait-for-CoW orders and to tell the frontend when the next batch starts.

use std::num::NonZeroU32;

/// When canonical batches start and which orders take part in them.
///
/// Batch times are the multiples of `interval` seconds since the Unix epoch,
/// so every replica computes the same times without coordinating.
///
/// An order first takes part in the first batch that starts more than
/// `cutoff` seconds after the order was created: an order placed in the last
/// `cutoff` seconds before a batch time joins the batch after it. This gives
/// the autopilot time to see every order of a batch (committed by the
/// orderbook or indexed from the chain) before it builds the batch, so the
/// batch an order joins is known when the order is placed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Schedule {
    interval: NonZeroU32,
    cutoff: u32,
}

impl Schedule {
    /// Creates a schedule. Returns `None` unless the cutoff is shorter than
    /// the interval.
    pub fn new(interval: NonZeroU32, cutoff: u32) -> Option<Self> {
        (cutoff < interval.get()).then_some(Self { interval, cutoff })
    }

    /// Seconds between two batch times.
    pub fn interval(&self) -> u32 {
        self.interval.get()
    }

    /// Orders created less than this many seconds before a batch time join
    /// the batch after it.
    pub fn cutoff(&self) -> u32 {
        self.cutoff
    }

    /// The latest batch time at or before `time`.
    pub fn latest_batch(&self, time: u32) -> u32 {
        time - time % self.interval
    }

    /// The first batch time strictly after `time`.
    pub fn next_batch(&self, time: u32) -> u32 {
        self.latest_batch(time).saturating_add(self.interval.get())
    }

    /// The first batch time an order created at `created` takes part in.
    pub fn first_batch(&self, created: u32) -> u32 {
        self.next_batch(created.saturating_add(self.cutoff))
    }
}

/// Reads the canonical batch schedule the autopilot published.
#[async_trait::async_trait]
pub trait ScheduleReading: Send + Sync {
    /// Returns the published schedule, or `None` if canonical batches are
    /// off.
    async fn schedule(&self) -> anyhow::Result<Option<Schedule>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_schedule(interval: u32, cutoff: u32) -> Schedule {
        Schedule::new(NonZeroU32::new(interval).unwrap(), cutoff).unwrap()
    }

    #[test]
    fn cutoff_must_be_shorter_than_interval() {
        let interval = NonZeroU32::new(60).unwrap();
        assert!(Schedule::new(interval, 59).is_some());
        assert!(Schedule::new(interval, 60).is_none());
        assert!(Schedule::new(interval, 61).is_none());
    }

    #[test]
    fn batch_times_are_multiples_of_the_interval() {
        let schedule = new_schedule(60, 5);

        assert_eq!(schedule.latest_batch(0), 0);
        assert_eq!(schedule.latest_batch(59), 0);
        assert_eq!(schedule.latest_batch(60), 60);
        assert_eq!(schedule.latest_batch(61), 60);

        // Strictly after: a time on a batch time gives the following one.
        assert_eq!(schedule.next_batch(0), 60);
        assert_eq!(schedule.next_batch(59), 60);
        assert_eq!(schedule.next_batch(60), 120);
    }

    #[test]
    fn orders_created_within_the_cutoff_join_the_batch_after() {
        let schedule = new_schedule(60, 5);

        // More than `cutoff` before the batch time: that batch.
        assert_eq!(schedule.first_batch(54), 60);
        // Exactly `cutoff` before it, or later: the batch after.
        assert_eq!(schedule.first_batch(55), 120);
        assert_eq!(schedule.first_batch(59), 120);
        // Created exactly on a batch time: the batch after.
        assert_eq!(schedule.first_batch(60), 120);
        assert_eq!(schedule.first_batch(61), 120);

        // Without a cutoff, any order created before the batch time joins it.
        let no_cutoff = new_schedule(60, 0);
        assert_eq!(no_cutoff.first_batch(59), 60);
        assert_eq!(no_cutoff.first_batch(60), 120);
    }

    #[test]
    fn saturates_instead_of_overflowing() {
        let schedule = new_schedule(60, 5);
        assert_eq!(schedule.next_batch(u32::MAX), u32::MAX);
        assert_eq!(schedule.first_batch(u32::MAX - 1), u32::MAX);
    }
}
