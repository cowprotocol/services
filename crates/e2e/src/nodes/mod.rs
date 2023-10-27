use bollard::{
    container::{Config, ListContainersOptions},
    service::HostConfig,
};

pub mod forked_node;
pub mod local_node;

/// A blockchain node for development purposes. Dropping this type will
/// terminate the node.
pub struct Node {
    container_id: String,
    pub url: reqwest::Url,
}

const FOUNDRY_IMAGE: &str = "ghcr.io/foundry-rs/foundry:latest";

impl Node {
    /// Spawns a new node that is forked from the given URL.
    pub async fn forked(fork: impl reqwest::IntoUrl) -> Self {
        Self::spawn_container(vec![
            "--port",
            "8545",
            "--host",
            "0.0.0.0",
            "--fork-url",
            fork.as_str(),
        ])
        .await
    }

    /// Spawns a new local test net with some default parameters.
    pub async fn new() -> Self {
        Self::spawn_container(vec![
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
        ])
        .await
    }

    /// Spawn a new node instance using the list of given arguments.
    async fn spawn_container(args: Vec<&str>) -> Self {
        let docker = bollard::Docker::connect_with_socket_defaults().unwrap();

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

        docker
            .start_container::<&str>(&container.id, None)
            .await
            .unwrap();

        let summary = docker
            .list_containers(Some(ListContainersOptions {
                filters: [("id".into(), vec![container.id.clone()])].into(),
                ..Default::default()
            }))
            .await
            .unwrap();

        let rpc_port = summary[0].ports.as_ref().unwrap()[0].public_port.unwrap();
        let url = format!("http://localhost:{rpc_port}").parse().unwrap();

        // TODO properly wait to for the node to be available.
        // Anvil needs some time before it's able to handle requests.
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        Self {
            container_id: container.id,
            url,
        }
    }

    /// Most reliable way to kill the process. If you get the chance to manually
    /// clean up the [`Node`] do it because the [`Drop::drop`]
    /// implementation can not be as reliable due to missing async support.
    pub async fn kill(&self) {
        let docker = bollard::Docker::connect_with_socket_defaults().unwrap();
        if let Err(err) = docker
            .kill_container::<&str>(&self.container_id, None)
            .await
        {
            tracing::error!(?err, "could not kill anvil container");
        }
    }
}
