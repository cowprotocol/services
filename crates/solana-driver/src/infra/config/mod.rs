//! Configuration of infrastructural components.

pub mod file;

use {
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
    /// Configured solver engines to query for solutions.
    pub solvers: Vec<Solver>,
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
