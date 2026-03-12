use {serde::Deserialize, std::time::Duration};

const fn default_timeout() -> Duration {
    Duration::from_secs(10)
}

#[derive(Debug, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case")]
pub struct HttpClient {
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
