use {
    super::metrics,
    std::collections::HashSet,
    tracing::{Subscriber, span::Id},
    tracing_subscriber::{Layer, layer::Context, registry::LookupSpan},
};

/// A tracing layer that tracks active span counts for monitoring span leaks.
/// Only tracks spans in the whitelist to control metric cardinality.
pub struct SpanMetricsLayer {
    /// Whitelist of span names to track. Empty set means track all spans.
    tracked_spans: HashSet<&'static str>,
}

impl SpanMetricsLayer {
    /// Creates a new SpanMetricsLayer with a whitelist of span names to track.
    ///
    /// # Arguments
    /// * `tracked_spans` - Names of spans to track. If empty, all spans are
    ///   tracked.
    pub fn new(tracked_spans: Vec<&'static str>) -> Self {
        Self {
            tracked_spans: tracked_spans.into_iter().collect(),
        }
    }

    /// Check if a span should be tracked based on its name.
    fn should_track(&self, span_name: &str) -> bool {
        self.tracked_spans.is_empty() || self.tracked_spans.contains(span_name)
    }
}

impl<S> Layer<S> for SpanMetricsLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let span_name = span.name();
            if self.should_track(span_name) {
                metrics::get()
                    .active_spans
                    .with_label_values(&[span_name])
                    .inc();
            }
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let span_name = span.name();
            if self.should_track(span_name) {
                metrics::get()
                    .active_spans
                    .with_label_values(&[span_name])
                    .dec();
            }
        }
    }
}

/// Returns the default list of important span names to track.
pub fn default_tracked_spans() -> Vec<&'static str> {
    vec![
        // // Mempool and settlement operations
        // "mempool",
        // "settle",
        // "revealing",
        // "settling",
        // // Auction and competition
        // "auction",
        // "solve",
        // "postprocessing",
        // "scoring",
        // // Encoding and simulation
        // "encoding",
        // "simulated",
        // // Cache and background tasks
        // "cache_maintenance",
        // "current_block_stream",
        // "token_fetcher",
        // "balance_cache",
    ]
}
