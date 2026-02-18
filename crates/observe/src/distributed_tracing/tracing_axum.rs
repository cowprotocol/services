use {
    crate::{request_id::request_id, tracing::HeaderExtractor},
    axum::http::Request,
    opentelemetry::{global, trace::TraceContextExt},
    tracing::{Span, field, info, info_span},
    tracing_opentelemetry::OpenTelemetrySpanExt,
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
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });
    let request_id = request_id(request.headers());

    let span = info_span!("http_request", ?request_id, trace_id = field::Empty);
    if let Err(err) = span.set_parent(parent_context) {
        tracing::error!(?err, "failed to set request parent span!");
    }
    {
        let _span = span.enter();
        info!(uri = %request.uri(), method = %request.method(), "HTTP request");
    }

    span
}
