//! Module implementing the shutdown signal handling.

#[cfg(unix)]
pub async fn signal_handler() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    // equivalent to Ctrl+C
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");

    tokio::select! {
        _ = sigterm.recv() => {
            tracing::info!("received SIGTERM signal, initiating graceful shutdown");
        }
        _ = sigint.recv() => {
            tracing::info!("received SIGINT signal, initiating graceful shutdown");
        }
    }
}

// Best-effort implementation for non-unix systems
#[cfg(not(unix))]
pub async fn signal_handler() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("received CTRL+C signal, initiating graceful shutdown");
}
