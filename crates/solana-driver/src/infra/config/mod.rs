//! Configuration of infrastructural components.

pub mod file;

use {
    solana_sdk::pubkey::Pubkey,
    std::{net::SocketAddr, time::Duration},
};

/// Configuration of infrastructural components.
#[derive(Debug)]
pub struct Config {
    pub chain: Chain,
    pub rpc: Rpc,
    pub http: Http,
    pub solvers: Vec<Solver>,
}

/// Solana chain configuration.
#[derive(Debug)]
pub struct Chain {
    pub settlement_program_id: Pubkey,
}

/// RPC client configuration.
#[derive(Debug)]
pub struct Rpc {
    pub endpoints: Vec<url::Url>,
    pub request_timeout: Duration,
}

/// HTTP API server configuration.
#[derive(Debug)]
pub struct Http {
    pub bind_address: SocketAddr,
}

/// A configured solver engine.
#[derive(Debug)]
pub struct Solver {
    pub name: String,
    pub endpoint: url::Url,
    pub max_in_flight: usize,
}
