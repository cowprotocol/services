//! This module implements 2 alloy transport layers to help
//! with instrumenting RPC calls. The [`LabelingLayer`] will "tag"
//! RPC calls that come through by adding the label to the call's
//! metadata. These layers can be stacked to generate arbitrarily
//! fine grained metrics.
//! The [`InstrumentationLayer`] reads that label metadata for each
//! call and emits logs and metrics with that label.
//! The module also exports the [`ProviderLabelingExt`] extension
//! trait to conveniently create a new `Provider` with an additional
//! [`LabelingLayer`].
use {
    crate::{Web3, alloy::RpcClientRandomIdExt},
    alloy::{
        providers::{Provider, ProviderBuilder},
        rpc::{
            client::RpcClient,
            json_rpc::{RequestPacket, ResponsePacket, SerializedRequest},
        },
        transports::TransportError,
    },
    std::{
        fmt::Debug,
        pin::Pin,
        task::{Context, Poll},
    },
    tower::{Layer, Service},
};

/// Layer that attaches a label to each request that passes through.
pub(crate) struct LabelingLayer {
    pub label: String,
}

impl<S> Layer<S> for LabelingLayer {
    type Service = LabeledProvider<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LabeledProvider {
            inner,
            // Append underscore for more readable labels
            // when multiple layers are nested in one another.
            // The last underscore will be dropped before
            // logging the final composed label.
            label: format!("{}_", self.label),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LabeledProvider<S> {
    inner: S,
    label: String,
}

impl<S> LabeledProvider<S> {
    fn attach_label(&self, req: &mut SerializedRequest) {
        req.meta_mut()
            .extensions_mut()
            .get_or_insert_default::<ProviderLabel>()
            .append(&self.label);
    }
}

impl<S> Service<RequestPacket> for LabeledProvider<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>,
    S::Future: Send + 'static,
    S::Response: Send + 'static + Debug,
    S::Error: Send + 'static + Debug,
{
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: RequestPacket) -> Self::Future {
        req.requests_mut()
            .iter_mut()
            .for_each(|r| self.attach_label(r));
        Box::pin(self.inner.call(req))
    }
}

/// Layer that logs and collects metrics based on the
/// [`ProviderLabel`] metadata attached to each request.
pub(crate) struct InstrumentationLayer;

impl<S> Layer<S> for InstrumentationLayer {
    type Service = InstrumentedProvider<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InstrumentedProvider {
            inner,
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InstrumentedProvider<S> {
    inner: S,
    metrics: &'static Metrics,
}

impl<S> Service<RequestPacket> for InstrumentedProvider<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>,
    S::Future: Send + 'static,
    S::Response: Send + 'static + Debug,
    S::Error: Send + 'static + Debug,
{
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: RequestPacket) -> Self::Future {
        let timers: Vec<_> = req
            .requests_mut()
            .iter_mut()
            .map(|r| {
                let component: String = r
                    .meta_mut()
                    .extensions_mut()
                    .remove::<ProviderLabel>()
                    .map(Into::into)
                    .unwrap_or_default();
                tracing::trace!(component, ?r, "executing request");
                self.metrics.on_request_start(&component, r.method())
            })
            .collect();

        if timers.len() > 1 {
            tracing::trace!(len = timers.len(), "executing batch request");
        }

        let fut = self.inner.call(req);
        Box::pin(async move {
            let res = fut.await;
            drop(timers);
            res
        })
    }
}

pub trait ProviderLabelingExt {
    /// Creates a new provider tagged with another label.
    fn labeled<S: ToString>(&self, label: S) -> Self;
}

impl ProviderLabelingExt for Web3 {
    fn labeled<S: ToString>(&self, label: S) -> Self {
        let is_local = self.alloy.client().is_local();
        let transport = self.alloy.client().transport().clone();
        let transport_with_label = LabelingLayer {
            label: label.to_string(),
        }
        .layer(transport);
        let client = RpcClient::with_random_id(transport_with_label, is_local);
        let alloy = ProviderBuilder::new()
            .wallet(self.wallet.clone())
            .connect_client(client)
            .erased();

        Self {
            alloy,
            wallet: self.wallet.clone(),
        }
    }
}

/// Label that identifies which component emitted a request.
/// Each [`LabelingLayer`] a request passes through appends its
/// own label to it so we know the entire hierarchy of components
/// a request went through.
#[derive(Debug, Clone)]
struct ProviderLabel(String);

impl Default for ProviderLabel {
    fn default() -> Self {
        // overallocate to avoid reallocations when other layers add more labels
        Self(String::with_capacity(30))
    }
}

impl ProviderLabel {
    fn append(&mut self, label: &str) {
        self.0.insert_str(0, label)
    }
}

impl From<ProviderLabel> for String {
    fn from(mut value: ProviderLabel) -> Self {
        // The labeling layer always appends an underscore
        // to the label. To have a clean identifier we pop
        // of the last one.
        value.0.pop();
        value.0
    }
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "alloy_rpc")]
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
    fn on_request_start(&self, label: &str, method: &str) -> impl Drop + use<> {
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
