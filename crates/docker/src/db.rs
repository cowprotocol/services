use {
    crate::ContainerRegistry,
    bollard::{
        container::{Config, ListContainersOptions},
        service::HostConfig,
    },
    futures::{FutureExt, StreamExt},
    reqwest::Url,
    sqlx::{Executor, PgConnection},
    std::{
        panic::AssertUnwindSafe,
        sync::atomic::{AtomicUsize, Ordering},
    },
    tokio::sync::Mutex,
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
                    cmd: Some(vec!["-d", "postgres", "-c", "log_statement=all"]),
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

/// Delete all data in the database. Only used by tests.
pub async fn clear_db(db: &mut PgConnection) {
    let tables = sqlx::query_scalar::<_, String>(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public';",
    )
    .fetch_all(db.as_mut())
    .await
    .unwrap();
    for table in tables {
        db.execute(format!("TRUNCATE TABLE {table};").as_str())
            .await
            .unwrap();
    }
}

/// Mutex to ensure that at most 1 test is using the DB container at any time.
static TEST_SETUP: Mutex<Option<TestSetup>> = Mutex::const_new(None);

/// Number of tests waiting to reuse the `TEST_SETUP` to know when to clean up.
static WAITING_TESTS: AtomicUsize = AtomicUsize::new(0);

struct TestSetup {
    registry: ContainerRegistry,
    db_url: reqwest::Url,
}

impl TestSetup {
    /// Spawns a new DB container and stores a handle to it.
    async fn new() -> Self {
        let registry = super::ContainerRegistry::default();
        let db_url = Db::new(&registry).await.url;
        Self { registry, db_url }
    }
}

/// Runs a database test using an empty dockerized DB.
pub async fn run_test<T, F>(test: T)
where
    T: FnOnce(Db) -> F,
    F: std::future::Future<Output = ()>,
{
    WAITING_TESTS.fetch_add(1, Ordering::Relaxed);
    let mut lock = TEST_SETUP.lock().await;
    if lock.is_none() {
        *lock = Some(TestSetup::new().await);
    }
    let setup = lock.as_ref().unwrap();

    let db = Db {
        // Reconnect to the singleton DB per test because we run into panics
        // if we share the [`Db`] directly.
        connection: sqlx::PgPool::connect(setup.db_url.as_str()).await.unwrap(),
        url: setup.db_url.clone(),
    };

    let mut tx = db.connection.begin().await.unwrap();
    clear_db(&mut tx).await;
    tx.commit().await.unwrap();

    // Catch panics to continue running other tests.
    let result = AssertUnwindSafe(test(db)).catch_unwind().await;

    if WAITING_TESTS.fetch_sub(1, Ordering::Relaxed) == 1 || result.is_err() {
        // Last test wanting to use `TEST_SETUP` exited or the test panicked.
        // In both cases we want to clean up and reset `TEST_SETUP`.
        lock.take()
            .expect("test setup initialized")
            .registry
            .kill_all()
            .await;
    }

    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
