use {
    crate::tracing::HeaderExtractor,
    opentelemetry::global,
    tracing_opentelemetry::OpenTelemetrySpanExt,
    warp::http::HeaderMap,
};

pub fn make_span(info: warp::trace::Info) -> tracing::Span {
    let headers: &HeaderMap = info.request_headers();

    // Extract OTEL context from headers
    let parent_cx = global::get_text_map_propagator(|prop| prop.extract(&HeaderExtractor(headers)));
    let request_id = headers
        .get("X-Request-Id")
        .and_then(|x| x.to_str().ok())
        .unwrap_or_default();

    let span = tracing::info_span!("http_request",
        method = %info.method(),
        path = %info.path(),
        request_id = %request_id,
    );

    span.set_parent(parent_cx); // sets parent context for distributed trace
    span
}
