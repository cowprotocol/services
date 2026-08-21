use {
    alloy::primitives::Address,
    anyhow::ensure,
    serde::{Deserialize, Deserializer, Serialize, de::Unexpected},
    std::collections::HashSet,
};

/// Configuration for per-order penalty caps (CIP-87).
///
/// The cap is a fraction of the order's volume, bounded
/// by an absolute USD amount.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PenaltyCapConfig {
    /// Fraction of an order's volume used as its penalty cap unless a
    /// bucket override applies.
    pub default_factor: PenaltyFactor,

    /// Upper bound for any order's penalty cap, denominated in USD.
    pub absolute_cap_usd: f64,

    /// Token whose native price is used to convert the USD bound into the
    /// native token (e.g. USDC).
    pub usd_reference_token: Address,

    /// Volume factor overrides for orders where both traded tokens belong
    /// to the same bucket (e.g. correlated tokens).
    #[serde(default)]
    pub overrides: Vec<PenaltyCapOverride>,
}

/// Penalty cap factor override for orders trading within a token bucket.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PenaltyCapOverride {
    /// Set of tokens forming the bucket.
    pub tokens: HashSet<Address>,

    /// Volume factor applied when both traded tokens are in the bucket.
    pub factor: PenaltyFactor,
}

/// Penalty cap volume factor in the range [0, 1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PenaltyFactor(f64);

impl PenaltyFactor {
    /// High precision scale factor (1 million) for sub-basis-point
    /// precision, allowing factors like 0.00001 (0.1 BPS) to be
    /// represented without rounding to 0.
    pub const HIGH_PRECISION_SCALE: u64 = 1_000_000;

    /// Converts the factor to a high precision scaled integer.
    /// For example, 0.00001 -> 10 (with scale of 1_000_000).
    pub fn to_high_precision(&self) -> u64 {
        (self.0 * Self::HIGH_PRECISION_SCALE as f64).round() as u64
    }

    /// Get the inner value
    pub fn get(&self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for PenaltyFactor {
    type Error = anyhow::Error;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        ensure!(
            (0.0..1.0).contains(&value),
            "Factor must be in the range [0, 1)"
        );
        Ok(PenaltyFactor(value))
    }
}

impl<'de> Deserialize<'de> for PenaltyFactor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = f64::deserialize(deserializer)?;
        PenaltyFactor::try_from(raw).map_err(|_| {
            serde::de::Error::invalid_value(
                Unexpected::Float(raw),
                &"expected penalty factor to be in interval [0, 1)",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::address};

    #[test]
    fn deserialize_penalty_cap_config() {
        let toml = r#"
        default-factor = 0.0004
        absolute-cap-usd = 20
        usd-reference-token = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"

        [[overrides]]
        factor = 0.00001
        tokens = [
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84",
        ]
        "#;
        let config: PenaltyCapConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.default_factor.get(), 0.0004);
        assert_eq!(config.absolute_cap_usd, 20.);
        assert_eq!(
            config.usd_reference_token,
            address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
        );
        assert_eq!(config.overrides.len(), 1);
        assert_eq!(config.overrides[0].factor.get(), 0.00001);
        assert_eq!(config.overrides[0].tokens.len(), 2);
    }
}
