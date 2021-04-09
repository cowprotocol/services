use std::{convert::Infallible, sync::Arc, time::Instant};

use prometheus::{HistogramOpts, HistogramVec, Registry};
use warp::Filter;
use warp::{reply::Response, Reply};

pub struct Metrics {
    requests: HistogramVec,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Self {
        let opts = HistogramOpts::new(
            "gp_v2_api_requests",
            "API Request durations labelled by route and response status code",
        );
        let requests = HistogramVec::new(opts, &["response", "request_type"]).unwrap();
        registry
            .register(Box::new(requests.clone()))
            .expect("Failed to register metric");
        Self { requests }
    }
}

// Response wrapper needed because we cannot inspect the reply's status code without consuming it
struct MetricsReply {
    response: Response,
}

impl Reply for MetricsReply {
    fn into_response(self) -> Response {
        self.response
    }
}

// Wrapper struct to annotate a reply with a handler label for logging purposes
pub struct LabelledReply {
    inner: Box<dyn Reply>,
    label: &'static str,
}

impl LabelledReply {
    pub fn new(inner: impl Reply + 'static, label: &'static str) -> Self {
        Self {
            inner: Box::new(inner),
            label,
        }
    }
}

impl Reply for LabelledReply {
    fn into_response(self) -> Response {
        self.inner.into_response()
    }
}

pub fn start_request() -> impl Filter<Extract = (Instant,), Error = Infallible> + Clone {
    warp::any().map(Instant::now)
}

pub fn end_request(metrics: Arc<Metrics>, timer: Instant, reply: LabelledReply) -> impl Reply {
    let LabelledReply { inner, label } = reply;
    let response = inner.into_response();
    let elapsed = timer.elapsed().as_secs_f64();
    metrics
        .requests
        .with_label_values(&[response.status().as_str(), label])
        .observe(elapsed);
    MetricsReply { response }
}
