use {
    anyhow::{Context, ensure},
    serde::{Deserialize, Deserializer, Serialize, de::Unexpected},
    std::str::FromStr,
};

/// Fee factor representing a percentage in range [0, 1)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FeeFactor(f64);

impl FeeFactor {
    /// High precision scale factor (1 million) for sub-basis-point precision.
    /// Allows representing factors like 0.00003 (0.3 BPS) without rounding to
    /// 0. Also used for converting to BPS string with 2 decimal precision
    /// (1_000_000 / 100 = 10_000 BPS scale).
    pub const HIGH_PRECISION_SCALE: u64 = 1_000_000;

    pub const fn new(factor: f64) -> Self {
        Self(factor)
    }

    /// Converts the fee factor to basis points (BPS).
    /// Supports fractional BPS values (e.g., 0.00003 -> "0.3")
    /// Rounds to 2 decimal places to avoid floating point representation
    /// issues.
    pub fn to_bps_str(&self) -> String {
        let bps = (self.0 * Self::HIGH_PRECISION_SCALE as f64).round() / 100.0;
        format!("{bps}")
    }

    /// Converts the fee factor to a high precision scaled integer.
    /// For example, 0.00003 -> 30 (with scale of 1_000_000)
    /// This allows sub-basis-point precision in calculations.
    pub fn to_high_precision(&self) -> u64 {
        (self.0 * Self::HIGH_PRECISION_SCALE as f64).round() as u64
    }

    /// Get the inner value
    pub fn get(&self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FeeFactor {
    type Error = anyhow::Error;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        ensure!(
            (0.0..1.0).contains(&value),
            "Factor must be in the range [0, 1)"
        );
        Ok(FeeFactor(value))
    }
}

impl FromStr for FeeFactor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: f64 = s.parse().context("failed to parse fee factor as f64")?;
        value.try_into()
    }
}

impl<'de> Deserialize<'de> for FeeFactor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw_fee_factor = f64::deserialize(deserializer)?;
        FeeFactor::try_from(raw_fee_factor).map_err(|_| {
            serde::de::Error::invalid_value(
                Unexpected::Float(raw_fee_factor),
                &"expected fee factor to be in interval [0, 1)",
            )
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn fee_factor_to_bps() {
        assert_eq!(FeeFactor::new(0.0001).to_bps_str(), "1");
        assert_eq!(FeeFactor::new(0.001).to_bps_str(), "10");

        // Fractional BPS values (sub-basis-point precision)
        assert_eq!(FeeFactor::new(0.00003).to_bps_str(), "0.3");
        assert_eq!(FeeFactor::new(0.00005).to_bps_str(), "0.5");
        assert_eq!(FeeFactor::new(0.000025).to_bps_str(), "0.25");
        assert_eq!(FeeFactor::new(0.000075).to_bps_str(), "0.75");
        assert_eq!(FeeFactor::new(0.00015).to_bps_str(), "1.5");

        assert_eq!(FeeFactor::new(0.0).to_bps_str(), "0");
    }

    #[test]
    fn fee_factor_to_high_precision() {
        // Verify high precision scaling
        assert_eq!(FeeFactor::new(0.00003).to_high_precision(), 30);
        assert_eq!(FeeFactor::new(0.0001).to_high_precision(), 100);
        assert_eq!(FeeFactor::new(0.001).to_high_precision(), 1000);
        assert_eq!(FeeFactor::new(0.01).to_high_precision(), 10_000);
        assert_eq!(FeeFactor::new(0.1).to_high_precision(), 100_000);
    }

    #[test]
    fn deserialize_valid_fee_factors() {
        assert_eq!(
            serde_json::from_str::<FeeFactor>("0.0").unwrap(),
            FeeFactor::new(0.0)
        );
        assert_eq!(
            serde_json::from_str::<FeeFactor>("0.5").unwrap(),
            FeeFactor::new(0.5)
        );
        assert_eq!(
            serde_json::from_str::<FeeFactor>("0.99").unwrap(),
            FeeFactor::new(0.99)
        );
        assert_eq!(
            serde_json::from_str::<FeeFactor>("0.00003").unwrap(),
            FeeFactor::new(0.00003)
        );
    }

    #[test]
    fn deserialize_invalid_fee_factors() {
        for value in ["1.0", "1.5", "-0.1", "-1.0"] {
            let err = serde_json::from_str::<FeeFactor>(value).unwrap_err();
            assert!(
                err.to_string()
                    .contains("expected fee factor to be in interval [0, 1)"),
                "unexpected error for {value}: {err}"
            );
        }
    }

    #[test]
    fn deserialize_wrong_type() {
        assert!(serde_json::from_str::<FeeFactor>("\"not a number\"").is_err());
        assert!(serde_json::from_str::<FeeFactor>("true").is_err());
    }
}
