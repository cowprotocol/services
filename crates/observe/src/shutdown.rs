//! Process shutdown signal handling.

/// Resolves once the process receives a shutdown signal. Kubernetes sends
/// SIGTERM, Ctrl-C sends SIGINT.
#[cfg(unix)]
pub async fn shutdown_signal() {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("install SIGINT handler");
    tokio::select! {
        _ = sigterm.recv() => (),
        _ = sigint.recv() => (),
    }
}

/// Signal handling is unsupported on Windows, the future never resolves.
#[cfg(windows)]
pub async fn shutdown_signal() {
    std::future::pending::<()>().await
}
