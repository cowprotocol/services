use {
    ethcontract::{
        dyns::DynWeb3,
        jsonrpc::types::{Call, Value},
        transport::DynTransport,
    },
    futures::{future::BoxFuture, FutureExt},
    std::sync::Arc,
    web3::{error::Error as Web3Error, BatchTransport, RequestId, Transport},
};

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "rpc")]
struct Metrics {
    /// Number of inflight RPC requests for ethereum node.
    #[metric(labels("component", "method"))]
    requests_inflight: prometheus::IntGaugeVec,

    /// Number of completed RPC requests for ethereum node.
    #[metric(labels("component", "method"))]
    requests_complete: prometheus::IntCounterVec,

    /// Execution time for each RPC request (batches are counted as one
    /// request).
    #[metric(labels("component", "method"))]
    requests_duration_seconds: prometheus::HistogramVec,

    /// Number of RPC requests initiated within a batch request
    #[metric(labels("component", "method"))]
    inner_batch_requests_initiated: prometheus::IntCounterVec,
}

impl Metrics {
    #[must_use]
    fn on_request_start(&self, label: &str, method: &str) -> impl Drop {
        let requests_inflight = self.requests_inflight.with_label_values(&[label, method]);
        let requests_complete = self.requests_complete.with_label_values(&[label, method]);
        let requests_duration_seconds = self
            .requests_duration_seconds
            .with_label_values(&[label, method]);

        requests_inflight.inc();
        let timer = requests_duration_seconds.start_timer();

        scopeguard::guard(timer, move |timer| {
            requests_inflight.dec();
            requests_complete.inc();
            timer.stop_and_record();
        })
    }
}

#[derive(Debug, Clone)]
pub struct InstrumentedTransport(Arc<Inner>);

impl InstrumentedTransport {
    pub fn new(label: String, transport: DynTransport) -> Self {
        Self(Arc::new(Inner {
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
            transport,
            label,
        }))
    }

    pub fn with_additional_label(&self, label: String) -> Self {
        Self(Arc::new(Inner {
            label: format!("{}_{label}", self.0.label),
            transport: self.0.transport.clone(),
            metrics: self.0.metrics,
        }))
    }
}

/// Adds metrics for RPC requests using the provided label.
pub fn instrument_with_label(web3: &DynWeb3, label: String) -> DynWeb3 {
    let transport = web3.transport().clone();
    let instrumented = if let Some(instrumented) = transport.downcast::<InstrumentedTransport>() {
        instrumented.with_additional_label(label)
    } else {
        InstrumentedTransport::new(label, transport)
    };
    web3::Web3::new(DynTransport::new(instrumented))
}

#[derive(Debug)]
struct Inner {
    metrics: &'static Metrics,
    transport: DynTransport,
    label: String,
}

type RpcResult = Result<Value, Web3Error>;

impl Transport for InstrumentedTransport {
    type Out = BoxFuture<'static, RpcResult>;

    fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
        self.0.transport.prepare(method, params)
    }

    fn send(&self, id: RequestId, call: Call) -> Self::Out {
        let inner = self.0.clone();

        async move {
            let _guard = inner
                .metrics
                .on_request_start(&inner.label, method_name(&call));
            inner.transport.send(id, call).await
        }
        .boxed()
    }
}

impl BatchTransport for InstrumentedTransport {
    type Batch = BoxFuture<'static, Result<Vec<RpcResult>, Web3Error>>;

    fn send_batch<R>(&self, requests: R) -> Self::Batch
    where
        R: IntoIterator<Item = (RequestId, Call)>,
    {
        // TODO: Can we get rid of this clone?
        // Do we even need the `BatchTransport` impl here?
        let inner = self.0.clone();
        let requests: Vec<_> = requests.into_iter().collect();

        async move {
            let _guard = inner.metrics.on_request_start(&inner.label, "batch");
            let metrics = inner.metrics;
            let label = &inner.label;

            let requests = requests.into_iter().inspect(move |(_id, call)| {
                metrics
                    .inner_batch_requests_initiated
                    .with_label_values(&[label, method_name(call)])
                    .inc()
            });

            inner.transport.send_batch(requests).await
        }
        .boxed()
    }
}

fn method_name(call: &Call) -> &str {
    match call {
        Call::MethodCall(method) => &method.method,
        Call::Notification(notification) => &notification.method,
        Call::Invalid { .. } => "invalid",
    }
}
