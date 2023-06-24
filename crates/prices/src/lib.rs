use {api::Api, clap::Parser, std::time::Duration};

mod api;
mod boundary;
mod cli;
mod config;
mod core;
mod util;

pub async fn main(args: impl Iterator<Item = String>) {
    let args = cli::Args::parse_from(args);
    let config = config::load(&args.config).await;
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();

    let mut estimators: Vec<Box<dyn core::Estimator>> = Vec::new();
    if let Some(zeroex) = config.zeroex {
        if zeroex.enable {
            estimators.push(Box::new(
                boundary::Zeroex {
                    api_key: zeroex.api_key,
                    endpoint: zeroex.endpoint,
                    timeout: std::time::Duration::from_millis(config.timeout_ms),
                }
                .estimator(),
            ));
        }
    }
    let serve = Api {
        estimators,
        addr: args.addr,
        addr_sender: None,
    }
    .serve(async {
        let _ = shutdown_receiver.await;
    });

    futures::pin_mut!(serve);
    tokio::select! {
        result = &mut serve => panic!("serve task exited: {result:?}"),
        _ = shutdown_signal() => {
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            match tokio::time::timeout(Duration::from_secs(10), serve).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => panic!("API shutdown exceeded timeout"),
            }
        }
    };
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept signals for graceful shutdown. Kubernetes sends sigterm, Ctrl-C
    // sends sigint.
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
    // No support for signal handling on Windows.
    std::future::pending().await
}
