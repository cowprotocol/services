use {
    bollard::{
        container::{Config, ListContainersOptions},
        service::HostConfig,
    },
    futures::StreamExt,
    reqwest::Url,
};

/// The docker image used to spawn the postgres container.
const POSTGRES_IMAGE: &str = "postgres:latest";

/// Handle to a dockerized postgres database.
#[derive(Debug)]
pub struct Db {
    container: String,
    url: Url,
    connection: DbConnection,
}

pub type DbConnection = sqlx::Pool<sqlx::Postgres>;

impl Db {
    /// Spawns a new dockerized postrgres database.
    pub async fn new() -> Self {
        let docker = bollard::Docker::connect_with_socket_defaults().unwrap();

        let postgres = docker
            .create_container::<&str, _>(
                None,
                Config {
                    image: Some(POSTGRES_IMAGE),
                    env: Some(vec![
                        "POSTGRES_HOST_AUTH_METHOD=trust",
                        "POSTGRES_USER=martin",
                        "POSTGRES_PASSWORD=123",
                    ]),
                    cmd: Some(vec!["-d", "postgres"]),
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
            .start_container::<&str>(&postgres.id, None)
            .await
            .unwrap();

        let summary = docker
            .list_containers(Some(ListContainersOptions {
                filters: [("id".into(), vec![postgres.id.clone()])].into(),
                ..Default::default()
            }))
            .await
            .unwrap();
        let db_port = summary[0].ports.as_ref().unwrap()[0].public_port.unwrap();

        let migrations = docker
            .create_container::<&str, _>(
                None,
                Config {
                    image: Some("migrations"),
                    cmd: Some(vec!["migrate"]),
                    env: Some(vec![&format!(
                        "FLYWAY_URL=jdbc:postgresql://localhost:{db_port}/?user=martin&password="
                    )]),
                    host_config: Some(HostConfig {
                        auto_remove: Some(true),
                        network_mode: Some("host".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        docker
            .start_container::<&str>(&migrations.id, None)
            .await
            .unwrap();

        // wait until migrations are done
        let _ = docker
            .wait_container::<&str>(&migrations.id, None)
            .next()
            .await;

        let url: Url = format!("http://postgres/127.0.0.1:{db_port}")
            .parse()
            .unwrap();
        Self {
            container: postgres.id.clone(),
            connection: sqlx::PgPool::connect(url.as_str()).await.unwrap(),
            url,
        }
    }

    /// Terminates the underlying docker container.
    pub async fn kill(&self) {
        let docker = bollard::Docker::connect_with_socket_defaults().unwrap();

        if let Err(err) = docker.kill_container::<&str>(&self.container, None).await {
            tracing::error!(?err, "failed to kill DB container");
        }
    }

    /// Returns the url used to connect to this database.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Returns a connection to the dockerized database.
    pub fn connection(&self) -> &DbConnection {
        &self.connection
    }
}
