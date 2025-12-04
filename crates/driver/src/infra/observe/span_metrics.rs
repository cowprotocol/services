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
    /// The hierarchy classification at the time of first enter.
    /// This must be stored because hierarchy can change (e.g., leaf ->
    /// intermediate) when children are created, but we need consistent
    /// inc/dec.
    hierarchy_at_enter: Option<&'static str>,
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
                hierarchy_at_enter: None,
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
                // Check if we've already determined hierarchy for this span
                let already_entered = span
                    .extensions()
                    .get::<SpanMetadata>()
                    .and_then(|m| m.hierarchy_at_enter)
                    .is_some();

                if !already_entered {
                    // First enter: determine and store hierarchy
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

                    // Store the hierarchy for consistent dec on exit
                    if let Some(metadata) = span.extensions_mut().get_mut::<SpanMetadata>() {
                        metadata.hierarchy_at_enter = Some(hierarchy);
                    }

                    metrics::get()
                        .active_spans_by_hierarchy
                        .with_label_values(&[span_name, hierarchy])
                        .inc();
                }
            }
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(&id) {
            let span_name = span.name();
            if self.should_track(span_name) {
                // Use the stored hierarchy from on_enter
                if let Some(hierarchy) = span
                    .extensions()
                    .get::<SpanMetadata>()
                    .and_then(|m| m.hierarchy_at_enter)
                {
                    metrics::get()
                        .active_spans_by_hierarchy
                        .with_label_values(&[span_name, hierarchy])
                        .dec();
                }
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
