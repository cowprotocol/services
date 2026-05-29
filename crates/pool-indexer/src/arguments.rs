use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env)]
    pub config: PathBuf,

    /// The log filter.
    #[clap(long, env, default_value = "warn,pool_indexer=debug,shared=debug")]
    pub log: String,

    /// At which log level logs should be printed to stderr instead of stdout.
    #[clap(long, env)]
    pub stderr_threshold: Option<tracing::Level>,

    /// Whether to use JSON format for the logs.
    #[clap(long, env, default_value = "false")]
    pub use_json_logs: bool,
}
