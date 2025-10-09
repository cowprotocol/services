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
        use tokio::signal;
        // On Unix-like systems, we can listen for SIGTERM.
        #[cfg(unix)]
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate()).unwrap();

        // On all platforms, we can listen for Ctrl+C.
        let ctrl_c = signal::ctrl_c();

        tokio::select! {
            _ = ctrl_c => {
                tracing::info!("Received SIGINT");
            },
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM.");
            },
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
