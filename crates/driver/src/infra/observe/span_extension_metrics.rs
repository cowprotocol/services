use super::metrics;

/// Track creation of a RequestId extension.
pub fn track_request_id_created(string_len: usize) {
    let size = i64::try_from(std::mem::size_of::<String>() + string_len).unwrap_or(i64::MAX);
    let metrics = metrics::get();
    metrics.span_extensions_memory_bytes.add(size);
    metrics
        .span_extensions_count
        .with_label_values(&["request_id"])
        .inc();
}

/// Track removal of a RequestId extension.
pub fn track_request_id_removed(string_len: usize) {
    let size = i64::try_from(std::mem::size_of::<String>() + string_len).unwrap_or(i64::MAX);
    let metrics = metrics::get();
    metrics.span_extensions_memory_bytes.sub(size);
    metrics
        .span_extensions_count
        .with_label_values(&["request_id"])
        .dec();
}
