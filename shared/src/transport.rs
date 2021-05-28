use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use derivative::Derivative;
use ethcontract::web3::{error, RequestId, Transport};
use ethcontract::{
    jsonrpc::types::{Call, Value},
    Http,
};
use futures::future::BoxFuture;
use futures::FutureExt;
use web3::BatchTransport;

/// Convenience method to create our standard instrumented transport
pub fn create_instrumented_transport<T>(
    transport: T,
    metrics: Arc<dyn TransportMetrics>,
) -> LoggingTransport<MetricTransport<T>>
where
    T: Transport,
    <T as Transport>::Out: Send + 'static,
{
    LoggingTransport::new(MetricTransport::new(transport, metrics))
}

/// Convenience method to create a compatible transport without metrics (noop)
pub fn create_test_transport(url: &str) -> LoggingTransport<MetricTransport<Http>>
where
{
    let transport = Http::new(url).expect("transport failure");
    LoggingTransport::new(MetricTransport::new(
        transport,
        Arc::new(NoopTransportMetrics),
    ))
}

#[derive(Debug, Clone)]
pub struct LoggingTransport<T: Transport> {
    inner: T,
}

impl<T: Transport> LoggingTransport<T> {
    pub fn new(inner: T) -> LoggingTransport<T> {
        Self { inner }
    }
}

impl<T> Transport for LoggingTransport<T>
where
    T: Transport,
    <T as Transport>::Out: Send + 'static,
{
    type Out = BoxFuture<'static, error::Result<Value>>;

    fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
        self.inner.prepare(method, params)
    }

    fn send(&self, id: RequestId, request: Call) -> Self::Out {
        if let Ok(serialized) = serde_json::to_string(&request) {
            tracing::debug!("[id:{}] sending request: '{}'", id, &serialized);
        }
        self.inner
            .send(id, request)
            .inspect(move |response| {
                match response {
                    Ok(value) => tracing::debug!("[id:{}] received response: '{}'", id, value),
                    Err(err) => tracing::debug!("[id:{}] returned an error: '{}'", id, err),
                };
            })
            .boxed()
    }
}

impl<T> BatchTransport for LoggingTransport<T>
where
    T: BatchTransport,
    T::Batch: Send + 'static,
    <T as Transport>::Out: Send + 'static,
{
    type Batch = BoxFuture<'static, error::Result<Vec<error::Result<Value>>>>;

    fn send_batch<I>(&self, requests: I) -> Self::Batch
    where
        I: IntoIterator<Item = (RequestId, Call)>,
    {
        let requests: Vec<_> = requests.into_iter().collect();
        // Empty batches are pointless and can therefore have a 0 id, otherwise we use the ID of the first request.
        let batch_id = requests.first().map(|(id, _)| *id).unwrap_or_default();
        tracing::debug!(
            "[batch_id:{}] sending Batch:\n{}",
            batch_id,
            requests
                .iter()
                .filter_map(|(_, call)| Some(format!("  {}", serde_json::to_string(call).ok()?)))
                .collect::<Vec<_>>()
                .join("\n")
        );
        self.inner
            .send_batch(requests.clone())
            .inspect(move |response| {
                match response {
                    Ok(responses) => tracing::debug!(
                        "[batch_id:{}] received response:\n{}",
                        batch_id,
                        responses
                            .iter()
                            .zip(requests.iter())
                            .map(|(response, request)| {
                                match response {
                                    Ok(v) => format!("  [id:{}]: {}", request.0, v),
                                    Err(e) => format!("  [id:{}]: {}", request.0, e),
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    Err(err) => {
                        tracing::debug!("[batch_id:{}] returned an error: '{}'", batch_id, err)
                    }
                };
            })
            .boxed()
    }
}

pub trait TransportMetrics: Send + Sync {
    fn report_query(&self, label: &str, elapsed: Duration);
}
#[derive(Clone, Derivative)]
#[derivative(Debug)]
pub struct MetricTransport<T: Transport> {
    inner: T,
    #[derivative(Debug = "ignore")]
    metrics: Arc<dyn TransportMetrics>,
}

impl<T: Transport> MetricTransport<T> {
    pub fn new(inner: T, metrics: Arc<dyn TransportMetrics>) -> MetricTransport<T> {
        Self { inner, metrics }
    }
}

impl<T> Transport for MetricTransport<T>
where
    T: Transport,
    <T as Transport>::Out: Send + 'static,
{
    type Out = BoxFuture<'static, error::Result<Value>>;

    fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
        self.inner.prepare(method, params)
    }

    fn send(&self, id: RequestId, request: Call) -> Self::Out {
        let metrics = self.metrics.clone();
        let start = Instant::now();
        self.inner
            .send(id, request.clone())
            .inspect(move |_| {
                let label = match request {
                    Call::MethodCall(method) => method.method,
                    Call::Notification(notification) => notification.method,
                    Call::Invalid { .. } => "invalid".into(),
                };
                metrics.report_query(&label, start.elapsed());
            })
            .boxed()
    }
}

impl<T> BatchTransport for MetricTransport<T>
where
    T: BatchTransport,
    T::Batch: Send + 'static,
    <T as Transport>::Out: Send + 'static,
{
    type Batch = BoxFuture<'static, error::Result<Vec<error::Result<Value>>>>;

    fn send_batch<I>(&self, requests: I) -> Self::Batch
    where
        I: IntoIterator<Item = (RequestId, Call)>,
    {
        let metrics = self.metrics.clone();
        let start = Instant::now();
        self.inner
            .send_batch(requests)
            .inspect(move |_| metrics.report_query(&"batch", start.elapsed()))
            .boxed()
    }
}

struct NoopTransportMetrics;
impl TransportMetrics for NoopTransportMetrics {
    fn report_query(&self, _: &str, _: Duration) {}
}
