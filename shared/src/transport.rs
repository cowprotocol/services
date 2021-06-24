pub mod dummy;
pub mod http;
pub mod instrumented;

use self::{
    http::HttpTransport,
    instrumented::{MetricTransport, TransportMetrics},
};
use ethcontract::web3::Transport;
use std::{convert::TryInto as _, sync::Arc, time::Duration};

/// Convenience method to create our standard instrumented transport
pub fn create_instrumented_transport<T>(
    transport: T,
    metrics: Arc<dyn TransportMetrics>,
) -> MetricTransport<T>
where
    T: Transport,
    <T as Transport>::Out: Send + 'static,
{
    MetricTransport::new(transport, metrics)
}

struct NoopTransportMetrics;
impl TransportMetrics for NoopTransportMetrics {
    fn report_query(&self, _: &str, _: Duration) {}
}

/// Convenience method to create a compatible transport without metrics (noop)
pub fn create_test_transport(url: &str) -> MetricTransport<HttpTransport>
where
{
    let transport = HttpTransport::new(url.try_into().unwrap());
    MetricTransport::new(transport, Arc::new(NoopTransportMetrics))
}

/// Like above but takes url from the environment NODE_URL.
pub fn create_env_test_transport() -> MetricTransport<HttpTransport>
where
{
    let env = std::env::var("NODE_URL").unwrap();
    let transport = HttpTransport::new(env.parse().unwrap());
    MetricTransport::new(transport, Arc::new(NoopTransportMetrics))
}
