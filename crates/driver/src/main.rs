use clap::Parser;
use driver::{api::serve_api, commit_reveal::CommitRevealSolver, driver::Driver};
use std::{sync::Arc, time::Duration};

#[tokio::main]
async fn main() {
    let args = driver::arguments::Arguments::parse();
    shared::tracing::initialize(args.log_filter.as_str(), args.log_stderr_threshold);
    tracing::info!("running driver with validated arguments:\n{}", args);

    let drivers = args
        .solvers
        .into_iter()
        .map(|arg| {
            let driver = Arc::new(Driver {
                solver: Arc::new(CommitRevealSolver {}),
            });
            (driver, arg.name)
        })
        .collect();

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve_api = serve_api(
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
        drivers,
    );

    futures::pin_mut!(serve_api);
    tokio::select! {
        result = &mut serve_api => tracing::error!(?result, "API task exited"),
        _ = shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            match tokio::time::timeout(Duration::from_secs(10), serve_api).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => tracing::error!("API shutdown exceeded timeout"),
            }
        }
    };
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept main signals for graceful shutdown
    // Kubernetes sends sigterm, whereas locally sigint (ctrl-c) is most common
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await
    };
    let sigint = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .unwrap()
            .recv()
            .await;
    };
    futures::pin_mut!(sigint);
    futures::pin_mut!(sigterm);
    futures::future::select(sigterm, sigint).await;
}

#[cfg(windows)]
async fn shutdown_signal() {
    // We don't support signal handling on windows
    std::future::pending().await
}
