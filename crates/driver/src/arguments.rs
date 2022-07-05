use solver::solver::ExternalSolverArg;
use std::net::SocketAddr;
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

    /// List of external solvers in the form of `name|url|account`.
    #[clap(long, env, use_value_delimiter = true)]
    pub external_solvers: Vec<ExternalSolverArg>,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "bind_address: {}", self.bind_address)?;
        writeln!(f, "log_filter: {}", self.log_filter)?;
        writeln!(f, "log_stderr_threshold: {}", self.log_stderr_threshold)?;
        write!(f, "external_solvers: {:?}", self.external_solvers)?;
        Ok(())
    }
}
