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
#[derive(Debug, Clone)]
pub struct Db {
    url: Url,
    connection: DbConnection,
}

pub type DbConnection = sqlx::Pool<sqlx::Postgres>;

impl Db {
    /// Spawns a new dockerized postrgres database.
    pub async fn new(registry: &super::ContainerRegistry) -> Self {
        let docker = bollard::Docker::connect_with_socket_defaults().unwrap();

        registry.pull_image(POSTGRES_IMAGE).await;

        let postgres = docker
            .create_container::<&str, _>(
                None,
                Config {
                    image: Some(POSTGRES_IMAGE),
                    env: Some(vec![
                        "POSTGRES_HOST_AUTH_METHOD=trust",
                        "POSTGRES_USER=admin",
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

        registry.start(postgres.id.clone()).await;

        let summary = docker
            .list_containers(Some(ListContainersOptions {
                filters: [("id".into(), vec![postgres.id.clone()])].into(),
                ..Default::default()
            }))
            .await
            .unwrap();
        let db_port = summary[0].ports.as_ref().unwrap()[0].public_port.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let migrations = docker
            .create_container::<&str, _>(
                None,
                Config {
                    image: Some("migrations"),
                    cmd: Some(vec!["migrate"]),
                    env: Some(vec![&format!(
                        "FLYWAY_URL=jdbc:postgresql://127.0.0.1:{db_port}/?user=admin&password="
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
            .expect(
                "Could not find \"migrations\" docker image. Run \"docker build -t \
                 migrations:latest -f ./docker/Dockerfile.migration .\" from the root of the \
                 repository.",
            );

        registry.start(migrations.id.clone()).await;

        // wait until migrations are done
        let _ = docker
            .wait_container::<&str>(&migrations.id, None)
            .next()
            .await;

        let url: Url = format!("postgres://127.0.0.1:{db_port}/?user=admin")
            .parse()
            .unwrap();

        Self {
            connection: sqlx::PgPool::connect(url.as_str()).await.unwrap(),
            url,
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
