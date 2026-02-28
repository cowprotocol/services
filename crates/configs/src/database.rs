use {
    std::{
        fmt::Debug,
        num::{NonZeroU32, NonZeroUsize},
        str::FromStr,
    },
    url::Url,
};

const fn default_db_max_connections() -> NonZeroU32 {
    // Matches SQLx default connection pool size.
    NonZeroU32::new(10).expect("value should be greater than 0")
}

fn default_db_write_url() -> Url {
    Url::from_str("postgresql://").expect("url should be valid")
}

const fn default_insert_batch_size() -> NonZeroUsize {
    NonZeroUsize::new(500).expect("value should be greater than 0")
}

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "test-util", derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DatabasePoolConfig {
    /// Url of the Postgres database. By default connects to `postgresql://`.
    /// Supports reading from an environment variable by prefixing the
    /// environment variable name with '%', for example — `%DB_WRITE_URL` —
    /// will read the environment variable named `DB_WRITE_URL`.
    #[serde(
        default = "default_db_write_url",
        deserialize_with = "crate::deserialize_env::deserialize_url_from_env"
    )]
    pub write_url: Url,

    /// Url of the Postgres database replica. If not provided, the URL from
    /// `write_url` will be used. Supports reading from an environment
    /// variable by prefixing the environment variable name with '%',
    /// for example — `%DB_READ_URL` — will read the environment variable named
    /// `DB_READ_URL`.
    #[serde(
        default,
        deserialize_with = "crate::deserialize_env::deserialize_optional_url_from_env"
    )]
    pub read_url: Option<Url>,

    /// Maximum number of connections in the database connection pool.
    #[serde(default = "default_db_max_connections")]
    pub max_connections: NonZeroU32,

    /// The number of order events to insert in a single batch.
    #[serde(default = "default_insert_batch_size")]
    pub insert_batch_size: NonZeroUsize,
}

impl Default for DatabasePoolConfig {
    fn default() -> Self {
        Self {
            write_url: default_db_write_url(),
            read_url: Default::default(),
            max_connections: default_db_max_connections(),
            insert_batch_size: default_insert_batch_size(),
        }
    }
}

impl Debug for DatabasePoolConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabasePoolConfig")
            .field("write_url", &"REDACTED")
            .field("read_url", &"REDACTED")
            .field("max_connections", &self.max_connections)
            .field("insert_batch_size", &self.insert_batch_size)
            .finish()
    }
}
