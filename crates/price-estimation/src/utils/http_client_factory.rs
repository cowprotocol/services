use {
    reqwest::{Client, ClientBuilder},
    std::time::Duration,
};

/// An HTTP client factory.
///
/// This ensures a common configuration for all our HTTP clients used in various
/// places, while allowing for separate configurations, connection pools, and
/// cookie stores (for things like sessions and default headers) across
/// different APIs.
#[derive(Clone, Debug)]
pub struct HttpClientFactory {
    timeout: Duration,
}

impl HttpClientFactory {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Creates a new HTTP client with the default settings.
    pub fn create(&self) -> Client {
        self.builder().build().unwrap()
    }

    /// Creates a new HTTP client, allowing for additional configuration.
    pub fn configure(&self, config: impl FnOnce(ClientBuilder) -> ClientBuilder) -> Client {
        config(self.builder()).build().unwrap()
    }

    /// Returns a `ClientBuilder` with the default settings.
    pub fn builder(&self) -> ClientBuilder {
        const USER_AGENT: &str = "cowprotocol-services/2.0.0";
        ClientBuilder::new()
            .timeout(self.timeout)
            .tcp_keepalive(Duration::from_secs(60))
            .user_agent(USER_AGENT)
    }
}

impl Default for HttpClientFactory {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
        }
    }
}
