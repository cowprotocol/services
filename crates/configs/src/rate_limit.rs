use {
    anyhow::ensure,
    std::{
        fmt::{Display, Formatter},
        time::{Duration, Instant},
    },
};

#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case")]
pub struct Strategy {
    /// Point in time until which incoming requests are dropped due to
    /// active rate limiting.
    #[serde(skip, default = "Instant::now")]
    pub drop_requests_until: Instant,
    /// How many requests got rate limited in a row.
    #[serde(skip)]
    pub times_rate_limited: u64,
    /// Multiplier applied to the back-off duration after each successive
    /// rate-limited response. Must be >= 1.0.
    pub back_off_growth_factor: f64,
    /// Initial back-off duration used after the first rate-limited response.
    #[serde(with = "humantime_serde")]
    pub min_back_off: Duration,
    /// Upper bound for the back-off duration regardless of growth factor.
    #[serde(with = "humantime_serde")]
    pub max_back_off: Duration,
}

impl Strategy {
    pub fn try_new(
        back_off_growth_factor: f64,
        min_back_off: Duration,
        max_back_off: Duration,
    ) -> anyhow::Result<Self> {
        ensure!(
            back_off_growth_factor.is_normal(),
            "back_off_growth_factor must be a normal f64"
        );
        ensure!(
            back_off_growth_factor >= 1.0,
            "back_off_growth_factor needs to be at least 1.0"
        );
        ensure!(
            min_back_off <= max_back_off,
            "min_back_off needs to be <= max_back_off"
        );
        Ok(Self {
            drop_requests_until: Instant::now(),
            times_rate_limited: 0,
            back_off_growth_factor,
            min_back_off,
            max_back_off,
        })
    }
}

impl Default for Strategy {
    fn default() -> Self {
        Self::try_new(1.0, Duration::default(), Duration::default()).unwrap()
    }
}

impl Display for Strategy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RateLimitingStrategy{{ min_back_off: {:?}, max_back_off: {:?}, growth_factor: {:?} }}",
            self.min_back_off, self.max_back_off, self.back_off_growth_factor
        )
    }
}
