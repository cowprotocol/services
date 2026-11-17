use {
    anyhow::{Context, ensure},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::{num::NonZeroU32, time::Duration},
};

/// Time between two canonical batches. Canonical batches start at the Unix
/// times that are a multiple of this interval.
///
/// Configured as a duration like `"5m"`. It must be a whole number of seconds,
/// greater than zero and at most `u32::MAX` seconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalBatchInterval(NonZeroU32);

impl CanonicalBatchInterval {
    pub fn seconds(self) -> NonZeroU32 {
        self.0
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
        assert_eq!(parse("1s").unwrap().seconds().get(), 1);
        assert_eq!(parse("5m").unwrap().seconds().get(), 300);
        assert_eq!(parse("4294967295s").unwrap().seconds().get(), u32::MAX);
    }

    #[test]
    fn rejects_invalid_intervals() {
        for interval in ["0s", "1500ms", "4294967296s", "five minutes"] {
            assert!(parse(interval).is_err(), "{interval}");
        }
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
