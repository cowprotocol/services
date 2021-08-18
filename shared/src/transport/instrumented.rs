use derivative::Derivative;
use ethcontract::jsonrpc::types::{Call, Value};
use ethcontract::web3::{error, BatchTransport, RequestId, Transport};
use futures::future::BoxFuture;
use futures::FutureExt;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

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
    T::Out: Send + 'static,
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
    T::Out: Send + 'static,
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
            .inspect(move |_| metrics.report_query("batch", start.elapsed()))
            .boxed()
    }
}
