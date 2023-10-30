use {
    super::ContainerRegistry,
    bollard::{
        container::{Config, ListContainersOptions},
        service::HostConfig,
    },
    reqwest::IntoUrl,
};

/// A dockerized blockchain node for testing purposes.
pub struct Node {
    pub port: u16,
}

const FOUNDRY_IMAGE: &str = "ghcr.io/foundry-rs/foundry:latest";

impl Node {
    /// Spawns a new node that is forked from the given URL.
    pub async fn forked(fork: impl IntoUrl, registry: &ContainerRegistry) -> Self {
        Self::spawn_container(
            vec![
                "--port",
                "8545",
                "--host",
                "0.0.0.0",
                "--fork-url",
                fork.as_str(),
            ],
            registry,
        )
        .await
    }

    /// Spawns a new local test net with some default parameters.
    pub async fn new(registry: &ContainerRegistry) -> Self {
        Self::spawn_container(
            vec![
                "--port",
                "8545",
                "--host",
                "0.0.0.0",
                "--gas-price",
                "1",
                "--gas-limit",
                "10000000",
                "--base-fee",
                "0",
                "--balance",
                "1000000",
                "--chain-id",
                "1",
                "--timestamp",
                "1577836800",
            ],
            registry,
        )
        .await
    }

    /// Spawn a new node instance using the list of given arguments.
    async fn spawn_container(args: Vec<&str>, registry: &ContainerRegistry) -> Self {
        let docker = bollard::Docker::connect_with_socket_defaults().unwrap();

        registry.pull_image(FOUNDRY_IMAGE).await;

        let container = docker
            .create_container::<&str, _>(
                None,
                Config {
                    image: Some(FOUNDRY_IMAGE),
                    entrypoint: Some(vec!["anvil"]),
                    cmd: Some(args),
                    // Expose anvil's default listening port so `publish_all_ports` will actually
                    // cause the dynamically allocated host port to show up when listing the
                    // container.
                    exposed_ports: Some([("8545/tcp", Default::default())].into()),
                    host_config: Some(HostConfig {
                        auto_remove: Some(true),
                        publish_all_ports: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        registry.start(container.id.clone()).await;

        let summary = docker
            .list_containers(Some(ListContainersOptions {
                filters: [("id".into(), vec![container.id.clone()])].into(),
                ..Default::default()
            }))
            .await
            .unwrap();

        let rpc_port = summary[0].ports.as_ref().unwrap()[0].public_port.unwrap();

        tokio::time::timeout(
            tokio::time::Duration::from_millis(10_000),
            Self::wait_until_node_ready(rpc_port),
        )
        .await
        .expect("timed out waiting for the node to get ready");

        Self { port: rpc_port }
    }

    /// The node might not be able to handle requests right after being spawned.
    /// To not fail tests due to synchronization issues we periodically query
    /// the node until it returned the first successful response.
    async fn wait_until_node_ready(port: u16) {
        let client = reqwest::Client::new();

        let query_node = || {
            client
                .post(format!("http://127.0.0.1:{port}"))
                .json(&serde_json::json!({
                    "id": 1,
                    "jsonrpc": "2.0",
                    "method": "web3_clientVersion"
                }))
                .send()
        };

        let start = std::time::Instant::now();

        while !query_node()
            .await
            .is_ok_and(|res| res.status().is_success())
        {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        tracing::debug!(start_up = ?start.elapsed(), "node is ready to use");
    }
}
