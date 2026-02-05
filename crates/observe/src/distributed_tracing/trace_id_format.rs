use {
    chrono::Utc,
    opentelemetry::trace::{TraceContextExt, TraceId},
    serde::ser::{SerializeMap, Serializer as _},
    std::{
        collections::{HashMap, hash_map::Entry},
        fmt,
        io,
    },
    tracing::{Event, Span, Subscriber},
    tracing_opentelemetry::OpenTelemetrySpanExt,
    tracing_serde::{AsSerde, fields::AsMap},
    tracing_subscriber::{
        fmt::{
            FmtContext,
            FormatEvent,
            FormatFields,
            FormattedFields,
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
///   "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
///   "spans": {
///     "spanName1": {
///       "field1": 123,
///       "field2": "abc"
///     }
///   }
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

            // serialize all parent span names and their fields
            if let Some(scope) = ctx.event_scope() {
                let mut parent_spans = HashMap::<String, serde_json::Value>::new();

                for span in scope.from_root() {
                    let current_span_fields: serde_json::Map<String, serde_json::Value> = span
                        .extensions()
                        .get::<FormattedFields<N>>()
                        .and_then(|fields| serde_json::from_str(fields.as_str()).ok())
                        .unwrap_or_default();

                    match parent_spans.entry(span.name().to_string()) {
                        Entry::Vacant(entry) => {
                            entry.insert(serde_json::Value::Object(current_span_fields));
                        }
                        Entry::Occupied(mut entry) => {
                            // the desired format does not preserve the hierarchy of spans
                            // so theoretically there could be nested spans with the same
                            // name so we merge the fields of all spans with the same name
                            //
                            // if there are duplicated fields the value of the span closest
                            // to the processed event wins
                            //
                            // also theoretically we could detect fields getting overwritten
                            // but we couldn't log that without causing a stack overflow so we
                            // don't
                            entry
                                .get_mut()
                                .as_object_mut()
                                .expect("fields get initialized with an object")
                                .extend(current_span_fields.into_iter())
                        }
                    }
                }

                serializer.serialize_entry("spans", &parent_spans)?;
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
