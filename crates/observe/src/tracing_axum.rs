use {
    crate::tracing::HeaderExtractor,
    axum::http::Request,
    opentelemetry::{global, trace::TraceContextExt},
    std::fmt::Debug,
    tracing::{Span, field, info_span},
    tracing_opentelemetry::OpenTelemetrySpanExt,
};

/// Trace context propagation: associate the current span with the OTel trace of
/// the given request, if any and valid.
pub fn accept_trace<B: Debug>(request: Request<B>) -> Request<B> {
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });

    Span::current().set_parent(parent_context);

    request
}

/// Record the OTel trace ID of the given request as "trace_id" field in the
/// current span.
pub fn record_trace_id<B>(request: Request<B>) -> Request<B> {
    let span = Span::current();
    let trace_id = span.context().span().span_context().trace_id();
    span.record("trace_id", trace_id.to_string());

    request
}

pub fn make_span<B>(request: &Request<B>) -> Span {
    let headers = request.headers();
    let uri = request.uri();
    let method = request.method();

    info_span!(
        "incoming request",
        ?headers,
        ?uri,
        ?method,
        trace_id = field::Empty
    )
}
