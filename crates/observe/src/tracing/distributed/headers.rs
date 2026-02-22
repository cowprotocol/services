use {
    axum::http::{self, HeaderMap},
    opentelemetry::{
        Context,
        global,
        propagation::{Extractor, Injector},
    },
    tracing::Span,
    tracing_opentelemetry::OpenTelemetrySpanExt,
};

pub struct HeaderExtractor<'a>(pub &'a HeaderMap);

// Copied from https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-http/src/lib.rs
// because that crate is using `http` crate v1 while warp is on v0.2
impl Extractor for HeaderExtractor<'_> {
    /// Get a value for a key from the HeaderMap.  If the value is not valid
    /// ASCII, returns None.
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    /// Collect all the keys from the HeaderMap.
    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|value| value.as_str())
            .collect::<Vec<_>>()
    }
}

pub struct HeaderInjector<'a>(pub &'a mut http::HeaderMap);

impl Injector for HeaderInjector<'_> {
    /// Set a key and value in the HeaderMap. Does nothing if the key or value
    /// are not valid inputs.
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) = (
            http::header::HeaderName::from_bytes(key.as_bytes()),
            http::header::HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, val);
        }
    }
}

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
