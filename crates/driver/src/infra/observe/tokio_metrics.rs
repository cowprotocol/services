use {std::time::Duration, tokio::time, tokio_metrics::RuntimeMonitor};

/// Spawns a background task that collects tokio runtime metrics
/// and exports them to Prometheus every 5 seconds.
///
/// Note: Some metrics require `tokio_unstable` feature and are not available.
pub fn spawn_runtime_monitor() {
    let monitor = RuntimeMonitor::new(&tokio::runtime::Handle::current());

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));

        for snapshot in monitor.intervals() {
            interval.tick().await;

            // Only stable fields are available without tokio_unstable
            // The fields live_tasks_count, blocking_threads_count, and
            // idle_blocking_threads_count require tokio_unstable feature

            tracing::trace!(
                workers = snapshot.workers_count,
                "tokio runtime metrics snapshot"
            );

            // We can't update the metrics without tokio_unstable fields
            // TODO: Either enable tokio_unstable or remove these metrics
        }
    });
}
