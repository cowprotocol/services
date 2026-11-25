//! Canonical batches are auctions that start at fixed times. Orders placed
//! with `enableWaitForCow` stay out of auctions until the next canonical
//! batch, so that the orders waiting for the same batch get a chance to be
//! matched with each other.

use {
    anyhow::{Context, ensure},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::{num::NonZeroU32, time::Duration},
};

/// Time between two canonical batches. Canonical batches start at the Unix
/// times that are a multiple of this interval, so every service computes the
/// same times without coordinating.
///
/// Configured as a duration like `"5m"`. It must be a whole number of seconds,
/// greater than zero and at most `u32::MAX` seconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalBatchInterval(NonZeroU32);

impl CanonicalBatchInterval {
    /// Seconds between two batch times.
    pub fn seconds(self) -> u32 {
        self.0.get()
    }

    /// The first batch time at or after `time`, in Unix seconds. Saturates at
    /// `u32::MAX`. Computed as modulo of the interval over the epoch so any
    /// component with a clock can calculate it without resorting to a
    /// centralised scheduler.
    pub fn next_batch(self, time: u32) -> u32 {
        let interval = self.0.get();
        match time % interval {
            0 => time,
            remainder => time.saturating_add(interval - remainder),
        }
    }
}

impl TryFrom<Duration> for CanonicalBatchInterval {
    type Error = anyhow::Error;

    fn try_from(interval: Duration) -> anyhow::Result<Self> {
        ensure!(
            interval.subsec_nanos() == 0,
            "must be a whole number of seconds"
        );
        let seconds =
            u32::try_from(interval.as_secs()).context("must be at most u32::MAX seconds")?;
        NonZeroU32::new(seconds)
            .map(Self)
            .context("must be greater than zero")
    }
}

impl Serialize for CanonicalBatchInterval {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        humantime_serde::serialize(&Duration::from_secs(self.0.get().into()), serializer)
    }
}

impl<'de> Deserialize<'de> for CanonicalBatchInterval {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let interval: Duration = humantime_serde::deserialize(deserializer)?;
        Self::try_from(interval).map_err(|err| {
            serde::de::Error::custom(format!(
                "invalid canonical batch interval {interval:?}: {err:#}"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, Serialize)]
    struct Config {
        interval: CanonicalBatchInterval,
    }

    fn parse(interval: &str) -> anyhow::Result<CanonicalBatchInterval> {
        Ok(toml::from_str::<Config>(&format!("interval = \"{interval}\""))?.interval)
    }

    #[test]
    fn accepts_whole_seconds() {
        assert_eq!(parse("1s").unwrap().seconds(), 1);
        assert_eq!(parse("5m").unwrap().seconds(), 300);
        assert_eq!(parse("4294967295s").unwrap().seconds(), u32::MAX);
    }

    #[test]
    fn rejects_invalid_intervals() {
        for interval in ["0s", "1500ms", "4294967296s", "five minutes"] {
            assert!(parse(interval).is_err(), "{interval}");
        }
    }

    fn interval(seconds: u64) -> CanonicalBatchInterval {
        Duration::from_secs(seconds).try_into().unwrap()
    }

    #[test]
    fn next_batch_is_the_first_multiple_of_the_interval_at_or_after_time() {
        let interval = interval(60);
        assert_eq!(interval.next_batch(0), 0);
        assert_eq!(interval.next_batch(1), 60);
        assert_eq!(interval.next_batch(59), 60);
        assert_eq!(interval.next_batch(60), 60);
        assert_eq!(interval.next_batch(61), 120);
    }

    #[test]
    fn every_second_is_a_batch_time_with_an_interval_of_one() {
        let interval = interval(1);
        assert_eq!(interval.next_batch(0), 0);
        assert_eq!(interval.next_batch(1_700_000_001), 1_700_000_001);
    }

    #[test]
    fn next_batch_saturates_instead_of_overflowing() {
        assert_eq!(interval(60).next_batch(u32::MAX), u32::MAX);
        assert_eq!(interval(u32::MAX.into()).next_batch(1), u32::MAX);
    }

    #[test]
    fn roundtrip_serialization() {
        let config = Config {
            interval: parse("5m").unwrap(),
        };
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.interval, config.interval);
    }
}
