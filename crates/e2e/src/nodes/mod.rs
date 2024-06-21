pub mod forked_node;
pub mod local_node;

/// The default node URL that should be used for e2e tests.
pub const NODE_HOST: &str = "http://127.0.0.1:8545";

/// A blockchain node for development purposes. Dropping this type will
/// terminate the node.
pub struct Node {
    process: Option<tokio::process::Child>,
}

impl Node {
    /// Spawns a new node that is forked from the given URL at `block_number` or
    /// if not set, latest.
    pub async fn forked(fork: impl reqwest::IntoUrl, block_number: Option<u64>) -> Self {
        let mut args = ["--port", "8545", "--fork-url", fork.as_str()]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();

        if let Some(block_number) = block_number {
            args.extend(["--fork-block-number".to_string(), block_number.to_string()]);
        }

        Self::spawn_process(args).await
    }

    /// Spawns a new local test net with some default parameters.
    pub async fn new() -> Self {
        Self::spawn_process(&[
            "--port",
            "8545",
            "--gas-price",
            "1",
            "--gas-limit",
            "10000000",
            "--base-fee",
            "1",
            "--balance",
            "1000000",
            "--chain-id",
            "1",
            "--timestamp",
            "1577836800",
            "--hardfork",
            "cancun",
        ])
        .await
    }

    /// Spawn a new node instance using the list of given arguments.
    async fn spawn_process<T>(args: impl IntoIterator<Item = T>) -> Self
    where
        T: AsRef<str> + std::convert::AsRef<std::ffi::OsStr>,
    {
        use tokio::io::AsyncBufReadExt as _;

        // Allow using some custom logic to spawn `anvil` by setting `ANVIL_COMMAND`.
        // For example if you set up a command that spins up a docker container.
        let command = std::env::var("ANVIL_COMMAND").unwrap_or("anvil".to_string());

        let mut process = tokio::process::Command::new(command)
            .args(args)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = process.stdout.take().unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel::<String>();

        tokio::task::spawn(async move {
            let mut sender = Some(sender);
            const NEEDLE: &str = "Listening on ";
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            while let Some(line) = reader.next_line().await.unwrap() {
                tracing::trace!(line);
                if let Some(addr) = line.strip_prefix(NEEDLE) {
                    match sender.take() {
                        Some(sender) => sender.send(format!("http://{addr}")).unwrap(),
                        None => tracing::error!(addr, "detected multiple anvil endpoints"),
                    }
                }
            }
        });

        let _url = tokio::time::timeout(tokio::time::Duration::from_secs(20), receiver)
            .await
            .expect("finding anvil URL timed out")
            .unwrap();

        Self {
            process: Some(process),
        }
    }

    /// Most reliable way to kill the process. If you get the chance to manually
    /// clean up the [`Node`] do it because the [`Drop::drop`]
    /// implementation can not be as reliable due to missing async support.
    pub async fn kill(&mut self) {
        let mut process = match self.process.take() {
            Some(node) => node,
            // Somebody already called `Node::kill()`
            None => return,
        };

        if let Err(err) = process.kill().await {
            tracing::error!(?err, "failed to kill node process");
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let mut process = match self.process.take() {
            Some(process) => process,
            // Somebody already called `Node::kill()`
            None => return,
        };

        // This only sends SIGKILL to the process but does not wait for the process to
        // actually terminate. But since `anvil` is fairly well behaved that
        // should be good enough in many cases.
        if let Err(err) = process.start_kill() {
            tracing::error!(?err, "failed to kill node process");
        }
    }
}
