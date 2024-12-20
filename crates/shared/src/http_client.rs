use {
    reqwest::{Client, ClientBuilder},
    std::{
        fmt::{self, Display, Formatter},
        time::Duration,
    },
};

const USER_AGENT: &str = "cowprotocol-services/2.0.0";

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
    pub fn new(args: &Arguments) -> Self {
        Self {
            timeout: args.http_timeout,
        }
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
        ClientBuilder::new()
            .timeout(self.timeout)
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

/// Command line arguments for the common HTTP factory.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// Default timeout in seconds for http requests.
    #[clap(
        long,
        env,
        default_value = "10s",
        value_parser = humantime::parse_duration,
    )]
    pub http_timeout: Duration,
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self { http_timeout } = self;

        writeln!(f, "http_timeout: {:?}", http_timeout)
    }
}
