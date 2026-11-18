//! Canonical batches are auctions that start at fixed times. Orders placed
//! with `enableWaitForCow` stay out of auctions until the next canonical
//! batch, so that the orders waiting for the same batch get a chance to be
//! matched with each other.

use {configs::canonical_batch::CanonicalBatchInterval, std::num::NonZeroU32};

/// When canonical batches start: at the Unix times that are a multiple of the
/// interval, so every service computes the same times without coordinating.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Schedule {
    interval: NonZeroU32,
}

impl Schedule {
    pub fn new(interval: NonZeroU32) -> Self {
        Self { interval }
    }

    /// Seconds between two batch times.
    pub fn interval(&self) -> u32 {
        self.interval.get()
    }

    /// The first batch time at or after `time`, in Unix seconds. Saturates at
    /// `u32::MAX`.
    pub fn next_batch(&self, time: u32) -> u32 {
        let interval = self.interval.get();
        match time % interval {
            0 => time,
            remainder => time.saturating_add(interval - remainder),
        }
    }
}

impl From<CanonicalBatchInterval> for Schedule {
    fn from(interval: CanonicalBatchInterval) -> Self {
        Self::new(interval.seconds())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(interval: u32) -> Schedule {
        Schedule::new(NonZeroU32::new(interval).unwrap())
    }

    #[test]
    fn next_batch_is_the_first_multiple_of_the_interval_at_or_after_time() {
        let schedule = schedule(60);
        assert_eq!(schedule.next_batch(0), 0);
        assert_eq!(schedule.next_batch(1), 60);
        assert_eq!(schedule.next_batch(59), 60);
        assert_eq!(schedule.next_batch(60), 60);
        assert_eq!(schedule.next_batch(61), 120);
    }

    #[test]
    fn every_second_is_a_batch_time_with_an_interval_of_one() {
        let schedule = schedule(1);
        assert_eq!(schedule.next_batch(0), 0);
        assert_eq!(schedule.next_batch(1_700_000_001), 1_700_000_001);
    }

    #[test]
    fn saturates_instead_of_overflowing() {
        assert_eq!(schedule(60).next_batch(u32::MAX), u32::MAX);
        assert_eq!(schedule(u32::MAX).next_batch(1), u32::MAX);
    }
}
