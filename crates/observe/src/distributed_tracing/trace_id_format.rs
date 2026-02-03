use {
    chrono::Utc,
    opentelemetry::trace::{TraceContextExt, TraceId},
    serde::ser::{SerializeMap, Serializer as _},
    std::{collections::HashMap, fmt, io},
    tracing::{Event, Span, Subscriber},
    tracing_opentelemetry::OpenTelemetrySpanExt,
    tracing_serde::{AsSerde, fields::AsMap},
    tracing_subscriber::{
        fmt::{
            FmtContext,
            FormatEvent,
            FormatFields,
            format::{Format, Full, Writer},
            time::FormatTime,
        },
        registry::LookupSpan,
    },
};

/// A custom `tracing_subscriber::fmt::FormatEvent` implementation for JSON log
/// formatting that attaches OpenTelemetry `trace_id` at the root level of each
/// log object.
///
/// This formatter is useful for environments where logs are ingested into
/// systems like Grafana Tempo, Loki, or other observability backends that
/// benefit from having distributed trace metadata available for correlation and
/// search.
///
/// Instead of nesting tracing metadata inside the "fields" key, it elevates
/// `trace_id` and `span_id` to top-level keys for easier indexing.
///
/// ## Example Output
/// ```json
/// {
///   "timestamp": "2025-07-04T12:58:56.138095625+00:00",
///   "level": "INFO",
///   "fields": {
///     "message": "finished processing with success",
///     "status": 200
///   },
///   "target": "warp::filters::trace",
///   "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736"
/// }
/// ```
pub struct TraceIdJsonFormat;

impl<S, N> FormatEvent<S, N> for TraceIdJsonFormat
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let meta = event.metadata();

        let mut visit = || {
            let mut serializer = serde_json::Serializer::new(WriteAdapter(&mut writer));
            let mut serializer = serializer.serialize_map(None)?;
            serializer.serialize_entry("timestamp", &Utc::now().to_rfc3339())?;
            serializer.serialize_entry("level", &meta.level().as_serde())?;
            serializer.serialize_entry("fields", &event.field_map())?;
            serializer.serialize_entry("target", meta.target())?;

            let current_span = tracing::Span::current();
            let context = current_span.context();
            let span_ref = context.span();
            let span_context = span_ref.span_context();

            let trace_id = span_context.trace_id();
            if trace_id != TraceId::INVALID {
                serializer.serialize_entry("trace_id", &trace_id.to_string())?;
            }

            // serialize entire parent span hierarchy and their fields
            if let Some(scope) = ctx.event_scope() {
                let mut spans = Vec::new();

                for span in scope.from_root() {
                    let mut span_json = HashMap::<String, String>::new();
                    span_json.insert("name".to_string(), span.name().to_string());

                    let extensions = span.extensions();
                    if let Some(fields) =
                        extensions.get::<tracing_subscriber::fmt::FormattedFields<N>>()
                    {
                        for tuple in fields.split(' ') {
                            if let Some((field, arg)) = tuple.split_once('=') {
                                span_json.insert(field.to_string(), arg.to_string());
                            }
                        }
                    }

                    spans.push(span_json);
                }

                if !spans.is_empty() {
                    serializer.serialize_entry("spans", &spans)?;
                }
            }

            serializer.end()
        };

        visit().map_err(|_| std::fmt::Error)?;
        writeln!(writer)
    }
}

struct WriteAdapter<'a>(pub(crate) &'a mut dyn std::fmt::Write);

impl<'a> io::Write for WriteAdapter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        self.0.write_str(&s).map_err(io::Error::other)?;
        Ok(s.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A layered formatter that prepends the current OpenTelemetry trace ID to each
/// human-readable log line, then delegates formatting to a wrapped default
/// formatter (`self.inner`).
///
/// ## Example Output
/// ```text
/// [trace_id=4bf92f3577b34da6a3ce929d0e0e4736] 2025-07-04T13:35:17.741Z  INFO \
/// http_request{method=GET path=/api/v1/version request_id=123}: \
/// warp::filters::trace: processing request
/// ```
///
/// If no `trace_id` is present in the current span, nothing is prepended.
pub struct TraceIdFmt<T: FormatTime> {
    pub(crate) inner: Format<Full, T>,
}

impl<S, N, T: FormatTime> FormatEvent<S, N> for TraceIdFmt<T>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let trace_id = Span::current().context().span().span_context().trace_id();
        let mut line = String::new();

        // now let the normal formatter do all the fancy stuff and dump it into a String
        let format_res = self.inner.format_event(ctx, Writer::new(&mut line), event);
        if trace_id != TraceId::INVALID {
            // remove the new line the default formatter added
            if line.ends_with('\n') {
                line.pop();
            }
            // append trace id and a newline
            line.push_str(&format!(" trace_id={trace_id}\n"));
        }
        writer.write_str(&line)?;
        format_res
    }
}
