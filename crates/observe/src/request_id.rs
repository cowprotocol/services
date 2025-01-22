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
    std::fmt,
    tracing::{
        field::{Field, Visit},
        span::Attributes,
        Id,
        Span,
        Subscriber,
    },
    tracing_subscriber::{layer::Context, registry::LookupSpan, Layer, Registry},
};

/// Name of the span that stores the id used to associated logs
/// across processes.
pub const SPAN_NAME: &str = "request";

pub fn info_span(request_id: String) -> Span {
    tracing::info_span!(SPAN_NAME, id = request_id)
}

/// Takes a `tower::Service` and embeds it in a `make_service` function that
/// spawns one of these services per incoming request.
/// But crucially before spawning that service task local storage will be
/// initialized with some request id.
/// Either that gets taken from the requests `X-REQUEST-ID` header of if that's
/// missing a globally unique request number will be generated.
#[macro_export]
macro_rules! make_service_with_task_local_storage {
    ($service:expr) => {{
        {
            let internal_request_id = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            hyper::service::make_service_fn(move |_| {
                let warp_svc = $service.clone();
                let internal_request_id = internal_request_id.clone();
                async move {
                    let svc =
                        hyper::service::service_fn(move |req: hyper::Request<hyper::Body>| {
                            let mut warp_svc = warp_svc.clone();
                            let id = if let Some(header) = req.headers().get("X-Request-ID") {
                                String::from_utf8_lossy(header.as_bytes()).to_string()
                            } else {
                                format!(
                                    "{}",
                                    internal_request_id
                                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                                )
                            };
                            let span = tracing::info_span!(observe::request_id::SPAN_NAME, id);
                            let task = hyper::service::Service::call(&mut warp_svc, req);
                            tracing::Instrument::instrument(task, span)
                        });
                    Ok::<_, std::convert::Infallible>(svc)
                }
            })
        }
    }};
}

/// Looks up the request id from the current tracing span.
pub fn from_current_span() -> Option<String> {
    let mut result = None;

    Span::current().with_subscriber(|(id, sub)| {
        let registry = sub.downcast_ref::<Registry>().unwrap();
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
pub struct ValuesLayer;

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for ValuesLayer {
    /// When creating a new span check if it contains the request_id and store
    /// it in the trace's extension storage to make it available for lookup
    /// later on.
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        if span.name() != crate::request_id::SPAN_NAME {
            return;
        }

        struct ValueVisitor(Option<RequestId>);
        impl Visit for ValueVisitor {
            // empty body because we want to use `record_str()` anyway
            fn record_debug(&mut self, _field: &Field, _value: &dyn fmt::Debug) {}

            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == "id" {
                    self.0 = Some(RequestId(value.to_string()));
                }
            }
        }

        let mut visitor = ValueVisitor(None);
        attrs.values().record(&mut visitor);

        if let Some(request_id) = visitor.0 {
            span.extensions_mut().insert(request_id);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn request_id_from_current_span() {}

    #[tokio::test]
    async fn request_id_from_parent_span() {}
}
