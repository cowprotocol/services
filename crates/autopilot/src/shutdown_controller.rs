pub struct ShutdownController {
    shutdown: tokio::sync::oneshot::Receiver<()>,
}

pub struct ShutdownSignal(tokio::sync::oneshot::Sender<()>);

impl ShutdownController {
    /// Creates a new Control which reacts to sigint/sigterm from the OS
    pub fn new_shutdown_on_signal() -> Self {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(Self::wait_for_signal(ShutdownSignal(sender)));
        Self { shutdown: receiver }
    }

    /// Creates a new Control that can be manually instructed to shut down
    /// the autopilot.
    pub fn new_manual_shutdown() -> (ShutdownSignal, Self) {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        (ShutdownSignal(sender), Self { shutdown: receiver })
    }

    async fn wait_for_signal(shutdown: ShutdownSignal) {
        #[cfg(unix)]
        {
            use tokio::{signal, signal::unix};
            // On Unix-like systems, we can listen for SIGTERM.
            let mut sigterm = unix::signal(unix::SignalKind::terminate()).unwrap();

            // Equivalent to SIGINT
            let ctrl_c = signal::ctrl_c();
            tokio::select! {
                _ = ctrl_c => {
                    tracing::info!("Received SIGINT");
                },
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM");
                },
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install CTRL+C handler");
            tracing::info!("Received SIGINT");
        }

        shutdown.shutdown();
    }

    /// Non-blocking check if shutdown signal has been received yet
    pub fn should_shutdown(&mut self) -> bool {
        self.shutdown.try_recv().is_ok()
    }
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new_shutdown_on_signal()
    }
}

impl ShutdownSignal {
    /// Send shutdown signal to the associated Control
    pub fn shutdown(self) {
        self.0.send(()).unwrap();
    }
}
