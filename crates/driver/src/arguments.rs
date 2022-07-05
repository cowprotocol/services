use reqwest::Url;
use shared::arguments::duration_from_seconds;
use solver::solver::ExternalSolverArg;
use std::{net::SocketAddr, time::Duration};
use tracing::level_filters::LevelFilter;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    #[clap(
        long,
        env,
        default_value = "warn,driver=debug,shared=debug,shared::transport::http=info"
    )]
    pub log_filter: String,

    #[clap(long, env, default_value = "error")]
    pub log_stderr_threshold: LevelFilter,

    /// List of solvers in the form of `name|url|account`.
    #[clap(long, env, use_value_delimiter = true)]
    pub solvers: Vec<ExternalSolverArg>,

    /// The Ethereum node URL to connect to.
    #[clap(long, env, default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// Timeout in seconds for all http requests.
    #[clap(
            long,
            default_value = "10",
            parse(try_from_str = duration_from_seconds),
        )]
    pub http_timeout: Duration,

    /// If solvers should use internal buffers to improve solution quality.
    #[clap(long, env)]
    pub use_internal_buffers: bool,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "bind_address: {}", self.bind_address)?;
        writeln!(f, "log_filter: {}", self.log_filter)?;
        writeln!(f, "log_stderr_threshold: {}", self.log_stderr_threshold)?;
        writeln!(f, "solvers: {:?}", self.solvers)?;
        writeln!(f, "node_url: {}", self.node_url)?;
        writeln!(f, "http_timeout: {:?}", self.http_timeout)?;
        write!(f, "use_internal_buffers: {}", self.use_internal_buffers)?;
        Ok(())
    }
}
