//! This module supplies the tools to associate 1 identifier with a task.
//! That identifier is accessable globally just for that task. The idea
//! is that this identifier is supposed to tie together related logs. That
//! is easy to accomplish in a single process (simply use a tracing span)
//! but if you want to tie together logs across multiple processes things
//! can get messier.
//! The most obvious option is to take that identifier and pass that through
//! the process until you make some request to another process and give that
//! process the identifier in your request.
//! However, if would do that the identifier would basically show up everywhere
//! although other components don't care about it and it doesn't even change
//! any behaviour in the process.
//! Instead we use task local storage that is globally visible but only
//! individual to each task. That way we can populate the storage with the
//! identifier once and not care about dragging it through the code base.
//! And when we issue requests to another process we can simply fetch the
//! current identifier specific to our task and send that along with the
//! request.

use {
    axum::http::{HeaderMap, HeaderValue},
    std::{
        fmt,
        sync::{OnceLock, atomic::AtomicUsize},
    },
    tracing::{
        Id,
        Span,
        Subscriber,
        field::{Field, Visit},
        span::Attributes,
    },
    tracing_subscriber::{Layer, Registry, layer::Context, registry::LookupSpan},
};

pub(crate) fn request_id(headers: &HeaderMap<HeaderValue>) -> String {
    static INSTANCE: OnceLock<AtomicUsize> = OnceLock::new();
    let counter = INSTANCE.get_or_init(|| AtomicUsize::new(0));

    if let Some(header) = headers.get("X-Request-ID") {
        String::from_utf8_lossy(header.as_bytes()).to_string()
    } else {
        format!(
            "{}",
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )
    }
}

/// Name of the span that stores the id used to associated logs
/// across processes.
pub const SPAN_NAME: &str = "request";

pub fn info_span(request_id: String) -> Span {
    tracing::info_span!(SPAN_NAME, id = request_id)
}

/// Looks up the request id from the current tracing span.
pub fn from_current_span() -> Option<String> {
    let mut result = None;

    Span::current().with_subscriber(|(id, sub)| {
        let Some(registry) = sub.downcast_ref::<Registry>() else {
            tracing::error!(
                "looking up request_ids using the `RequestIdLayer` requires the global tracing \
                 subscriber to be `tracing_subscriber::Registry`"
            );
            return;
        };
        let mut current_span = registry.span(id);
        while let Some(span) = current_span {
            if let Some(request_id) = span.extensions().get::<RequestId>() {
                result = Some(request_id.0.clone());
                return;
            }
            current_span = span.parent();
        }
    });

    result
}

/// Request id recovered from a tracing span.
struct RequestId(String);

/// Tracing layer that allows us to recover the request id
/// from the current tracing span.
pub struct RequestIdLayer;

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for RequestIdLayer {
    /// When creating a new span check if it contains the request_id and store
    /// it in the trace's extension storage to make it available for lookup
    /// later on.
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        if span.name() != SPAN_NAME {
            return;
        }

        struct RequestIdVisitor(Option<RequestId>);
        impl Visit for RequestIdVisitor {
            // empty body because we want to use `record_str()` anyway
            fn record_debug(&mut self, _field: &Field, _value: &dyn fmt::Debug) {}

            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == "id" {
                    self.0 = Some(RequestId(value.to_string()));
                }
            }
        }

        let mut visitor = RequestIdVisitor(None);
        attrs.values().record(&mut visitor);

        if let Some(request_id) = visitor.0 {
            span.extensions_mut().insert(request_id);
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, crate::config::Config, tracing::Instrument};

    fn init_tracing(env_filter: &str) {
        let obs_config = Config::new(env_filter, tracing::Level::ERROR.into(), false, None);
        crate::tracing::init::initialize_reentrant(&obs_config);
    }

    #[tokio::test]
    async fn request_id_from_current_span() {
        init_tracing("error");
        async {
            assert_eq!(Some("test".to_string()), from_current_span());
        }
        .instrument(info_span("test".to_string()))
        .await
    }

    #[tokio::test]
    async fn request_id_not_set() {
        init_tracing("debug");
        async {
            assert_eq!(None, from_current_span());
        }
        .await
    }

    #[tokio::test]
    async fn request_id_from_ancestor_span() {
        init_tracing("error");
        async {
            async {
                async {
                    // we traverse the span hierarchy until we find a span with the request id
                    assert_eq!(Some("test".to_string()), from_current_span());
                }
                .instrument(tracing::info_span!("wrap2", value = "value2"))
                .await
            }
            .instrument(tracing::info_span!("wrap1", value = "value1"))
            .await
        }
        .instrument(info_span("test".to_string()))
        .await
    }

    #[tokio::test]
    async fn request_id_from_first_ancestor_span() {
        init_tracing("error");
        async {
            async {
                async {
                    // if multiple ancestors have a request id we take the closest one
                    assert_eq!(Some("test_inner".to_string()), from_current_span());
                }
                .instrument(tracing::info_span!("wrap", value = "value"))
                .await
            }
            .instrument(info_span("test_inner".to_string()))
            .await
        }
        .instrument(info_span("test".to_string()))
        .await
    }

    #[tokio::test]
    async fn request_id_within_spawned_task() {
        init_tracing("error");
        async {
            tokio::spawn(
                async {
                    // we can spawn a new task and still find the request id if the spawned task
                    // was instrumented with a span that contains the request id
                    assert_eq!(Some("test".to_string()), from_current_span());
                }
                .instrument(Span::current()),
            )
            .await
            .unwrap();
        }
        .instrument(info_span("test".to_string()))
        .await
    }

    #[test]
    fn returns_existing_request_id_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Request-ID", HeaderValue::from_static("abc123"));

        let id = request_id(&headers);
        assert_eq!(id, "abc123");
    }

    #[test]
    fn generates_incrementing_request_id_when_header_missing() {
        let headers = HeaderMap::new();

        let id1 = request_id(&headers);
        let id2 = request_id(&headers);
        let id3 = request_id(&headers);

        let n1: usize = id1.parse().unwrap();
        let n2: usize = id2.parse().unwrap();
        let n3: usize = id3.parse().unwrap();

        assert!(n2 > n1);
        assert!(n3 > n2);
    }
}
