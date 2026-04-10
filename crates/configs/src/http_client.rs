use {serde::Deserialize, std::time::Duration};

const fn default_timeout() -> Duration {
    Duration::from_secs(10)
}

/// Global HTTP client settings shared across all outgoing requests.
#[derive(Debug, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case")]
pub struct HttpClient {
    /// Request timeout for outgoing HTTP calls.
    #[serde(with = "humantime_serde", default = "default_timeout")]
    pub timeout: Duration,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
        }
    }
}
