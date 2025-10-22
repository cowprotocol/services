pub mod forked_node;
pub mod local_node;

use anyhow::Context;

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
        let mut args = [
            "--port",
            "8545",
            "--fork-url",
            fork.as_str(),
            "--retries",
            "10",
            "--timeout",
            "120000",
            "--fork-retry-backoff",
            "2000",
        ]
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

        let url = tokio::time::timeout(tokio::time::Duration::from_secs(20), receiver)
            .await
            .expect("finding anvil URL timed out")
            .unwrap();

        wait_until_ready(&url)
            .await
            .unwrap_or_else(|err| panic!("anvil at {url} failed to become ready: {err}"));

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

async fn wait_until_ready(url: &str) -> anyhow::Result<()> {
    use std::time::Duration;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("building readiness client")?;
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_chainId",
        "params": [],
        "id": "node_ready_probe"
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        match client.post(url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                return Ok(());
            }
            Ok(resp) => {
                tracing::debug!(
                    "anvil readiness attempt {} returned status {}",
                    attempt,
                    resp.status()
                );
            }
            Err(err) => {
                tracing::debug!("anvil readiness attempt {} failed: {err}", attempt);
            }
        }

        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("anvil endpoint at {url} did not respond in time");
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
