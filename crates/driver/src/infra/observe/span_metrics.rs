use {
    super::metrics,
    std::collections::HashSet,
    tracing::{
        Subscriber,
        span::{Attributes, Id},
    },
    tracing_subscriber::{Layer, layer::Context, registry::LookupSpan},
};

/// Metadata stored in span extensions to track hierarchy information.
struct SpanMetadata {
    has_children: bool,
}

/// A tracing layer that tracks active span counts by hierarchy for monitoring
/// span leaks. Only tracks spans in the whitelist to control metric
/// cardinality.
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
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            // Initialize metadata for this span
            span.extensions_mut().insert(SpanMetadata {
                has_children: false,
            });

            // Mark parent as having children
            if let Some(parent) = span.parent()
                && let Some(metadata) = parent.extensions_mut().get_mut::<SpanMetadata>()
            {
                metadata.has_children = true;
            }
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let span_name = span.name();
            if self.should_track(span_name) {
                // Determine hierarchy category
                let has_parent = span.parent().is_some();
                let has_children = span
                    .extensions()
                    .get::<SpanMetadata>()
                    .map(|m| m.has_children)
                    .unwrap_or(false);

                let hierarchy = match (has_parent, has_children) {
                    (false, _) => "root",           // No parent = root/orphan
                    (true, false) => "leaf",        // Has parent but no children = leaf
                    (true, true) => "intermediate", // Has both = intermediate
                };

                metrics::get()
                    .active_spans_by_hierarchy
                    .with_label_values(&[span_name, hierarchy])
                    .inc();
            }
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let span_name = span.name();
            if self.should_track(span_name) {
                // Determine hierarchy category (same logic as on_enter)
                let has_parent = span.parent().is_some();
                let has_children = span
                    .extensions()
                    .get::<SpanMetadata>()
                    .map(|m| m.has_children)
                    .unwrap_or(false);

                let hierarchy = match (has_parent, has_children) {
                    (false, _) => "root",
                    (true, false) => "leaf",
                    (true, true) => "intermediate",
                };

                metrics::get()
                    .active_spans_by_hierarchy
                    .with_label_values(&[span_name, hierarchy])
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
