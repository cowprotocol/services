use {super::metrics, std::time::Duration, tokio::time, tokio_metrics::RuntimeMonitor};

/// Spawns a background task that collects tokio runtime metrics
/// and exports them to Prometheus every 5 seconds.
pub fn spawn_runtime_monitor() {
    let monitor = RuntimeMonitor::new(&tokio::runtime::Handle::current());

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));

        for snapshot in monitor.intervals() {
            interval.tick().await;

            let metrics = metrics::get();

            let Ok(live_tasks) = snapshot.live_tasks_count.try_into() else {
                tracing::error!(
                    count = snapshot.live_tasks_count,
                    "failed to convert live_tasks_count to i64"
                );
                continue;
            };

            let Ok(idle_workers) = snapshot.idle_blocking_threads_count.try_into() else {
                tracing::error!(
                    count = snapshot.idle_blocking_threads_count,
                    "failed to convert idle_blocking_threads_count to i64"
                );
                continue;
            };

            let Ok(active_workers) =
                (snapshot.blocking_threads_count - snapshot.idle_blocking_threads_count).try_into()
            else {
                tracing::error!(
                    blocking = snapshot.blocking_threads_count,
                    idle = snapshot.idle_blocking_threads_count,
                    "failed to convert active blocking threads to i64"
                );
                continue;
            };

            metrics.tokio_active_tasks.set(live_tasks);
            metrics.tokio_idle_workers.set(idle_workers);
            metrics.tokio_active_workers.set(active_workers);
        }
    });
}
