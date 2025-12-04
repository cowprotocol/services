use {
    super::span_extension_metrics,
    std::fmt,
    tracing::{
        Id,
        Subscriber,
        field::{Field, Visit},
        span::Attributes,
    },
    tracing_subscriber::{Layer, layer::Context, registry::LookupSpan},
};

/// Request id recovered from a tracing span.
struct RequestId(String);

/// Name of the span that stores the id used to associated logs
/// across processes.
const SPAN_NAME: &str = "request";

/// Tracing layer that tracks RequestId extensions and reports metrics.
/// This is similar to observe::distributed_tracing::request_id::RequestIdLayer
/// but also tracks memory usage.
pub struct RequestIdLayer;

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for RequestIdLayer {
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
            let string_len = request_id.0.len();
            span_extension_metrics::track_request_id_created(string_len);
            span.extensions_mut().insert(request_id);
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(&id)
            && span.name() == SPAN_NAME
            && let Some(request_id) = span.extensions().get::<RequestId>()
        {
            let string_len = request_id.0.len();
            span_extension_metrics::track_request_id_removed(string_len);
        }
    }
}
