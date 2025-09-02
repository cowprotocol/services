use {
    crate::{request_id::request_id, tracing::HeaderExtractor},
    axum::http::Request,
    opentelemetry::{global, trace::TraceContextExt},
    tracing::{Span, field, info, info_span},
    tracing_opentelemetry::OpenTelemetrySpanExt,
    warp::http as warp_http,
};

/// Record the OTel trace ID of the given request as "trace_id" field in the
/// current span.
pub fn record_trace_id<B>(request: Request<B>) -> Request<B> {
    let span = Span::current();
    let trace_id = span.context().span().span_context().trace_id();
    span.record("trace_id", trace_id.to_string());
    request
}

/// Trace context propagation: associate the current span with the OTel trace of
/// the given request, if any and valid.
pub fn make_span<B>(request: &Request<B>) -> Span {
    // Convert headers for helpers that expect http v0.2
    let headers_v0 = to_warp_headermap(request.headers());

    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(&headers_v0))
    });

    let request_id = request_id(&headers_v0);

    let span = info_span!("http_request", ?request_id, trace_id = field::Empty);
    span.set_parent(parent_context);
    {
        let _span = span.enter();
        info!(uri = %request.uri(), method = %request.method(), "HTTP request");
    }

    span
}

/// Convert axum/http v1 HeaderMap -> warp/http v0.2 HeaderMap
fn to_warp_headermap(h1: &axum::http::HeaderMap) -> warp_http::HeaderMap {
    let mut h0 = warp_http::HeaderMap::with_capacity(h1.len());
    for (k, v) in h1.iter() {
        // Convert key
        let k0 = k
            .as_str()
            .parse::<warp_http::HeaderName>()
            .expect("valid header name");
        // Convert value
        let v0 = warp_http::HeaderValue::from_bytes(v.as_bytes()).expect("valid header value");
        h0.append(k0, v0);
    }
    h0
}
