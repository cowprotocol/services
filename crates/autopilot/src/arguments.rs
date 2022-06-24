use std::net::SocketAddr;
use tracing::level_filters::LevelFilter;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env, default_value = "warn,autopilot=debug,shared=debug")]
    pub log_filter: String,

    #[clap(long, env, default_value = "error", parse(try_from_str))]
    pub log_stderr_threshold: LevelFilter,

    #[clap(long, env, default_value = "0.0.0.0:9589")]
    pub metrics_address: SocketAddr,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "log_filter: {}", self.log_filter)?;
        writeln!(f, "log_stderr_threshold: {}", self.log_stderr_threshold)?;
        writeln!(f, "metrics_address: {}", self.metrics_address)?;
        Ok(())
    }
}
