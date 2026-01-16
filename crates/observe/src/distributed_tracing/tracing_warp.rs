use {
    crate::{distributed_tracing::request_id::request_id, tracing::HeaderExtractor},
    opentelemetry::global,
    tracing::info,
    tracing_opentelemetry::OpenTelemetrySpanExt,
    warp::http::HeaderMap,
};

pub fn make_span(info: warp::trace::Info) -> tracing::Span {
    let headers: &HeaderMap = info.request_headers();

    // Extract OTEL context from headers
    let parent_cx = global::get_text_map_propagator(|prop| prop.extract(&HeaderExtractor(headers)));

    let span = tracing::info_span!("http_request", request_id = %request_id(headers));
    span.set_parent(parent_cx); // sets parent context for distributed trace
    {
        let _span = span.enter();
        info!(method = %info.method(), path = %info.path(), "HTTP request");
    }

    span
}
