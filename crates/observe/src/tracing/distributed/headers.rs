use {
    axum::http::HeaderMap,
    opentelemetry::{Context, global},
    opentelemetry_http::HeaderInjector,
    tracing::Span,
    tracing_opentelemetry::OpenTelemetrySpanExt,
};

pub fn tracing_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    Context::current();
    let span = Span::current();
    let cx = span.context();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut HeaderInjector(&mut headers))
    });

    headers
}
