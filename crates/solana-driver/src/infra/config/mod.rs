//! Configuration of infrastructural components.

pub mod file;

use {
    configs::shared::LoggingConfig,
    solana_sdk::pubkey::Pubkey,
    std::{net::SocketAddr, time::Duration},
};

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    /// Chain and deployment-specific configuration.
    pub chain: Chain,
    /// RPC client configuration.
    pub rpc: Rpc,
    /// HTTP API server configuration.
    pub http: Http,
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Configured solver engines to query for solutions.
    pub solvers: Vec<Solver>,
}

impl Config {
    /// Build the `observe::Config` for the tracing framework from the logging
    /// configuration.
    pub fn observe_config(&self) -> observe::Config {
        observe::Config::new(
            &self.logging.filter,
            self.logging.stderr_threshold,
            self.logging.use_json,
            None,
        )
    }
}

/// Solana chain configuration.
#[derive(Debug)]
pub struct Chain {
    /// On-chain program id of the settlement contract.
    pub settlement_program_id: Pubkey,
}

/// RPC client configuration.
#[derive(Debug)]
pub struct Rpc {
    /// RPC endpoints to connect to.
    pub endpoints: Vec<url::Url>,
    /// Timeout for individual RPC requests.
    pub request_timeout: Duration,
}

/// HTTP API server configuration.
#[derive(Debug)]
pub struct Http {
    /// Address the HTTP API server binds to and listens on.
    pub bind_address: SocketAddr,
}

/// A configured solver engine.
#[derive(Debug)]
pub struct Solver {
    /// Human-readable name identifying this solver, used for logging and
    /// metrics.
    pub name: String,
    /// HTTP endpoint of the solver engine API.
    pub endpoint: url::Url,
    /// Maximum number of concurrent solve requests kept in flight per solver.
    pub max_in_flight: usize,
}
