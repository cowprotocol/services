pub use crate::boundary::mempool::{Config, GlobalTxPool, Kind, RevertProtection, SubmissionLogic};

#[derive(Debug, Clone)]
pub enum Mempool {
    /// Legacy implementation of the mempool, using the shared and solvers crate
    Boundary(crate::boundary::mempool::Mempool),
    /// Driver native mempool implementation
    Native(Inner),
}

impl std::fmt::Display for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boundary(mempool) => write!(f, "Boundary({mempool})"),
            Self::Native(mempool) => write!(f, "Native({mempool})"),
        }
    }
}

impl Mempool {
    pub fn config(&self) -> &Config {
        match self {
            Self::Boundary(mempool) => mempool.config(),
            Self::Native(mempool) => &mempool.config,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Inner {
    config: Config,
}

impl std::fmt::Display for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mempool({})", self.config.kind.format_variant())
    }
}

impl Inner {
    pub fn config(&self) -> &Config {
        &self.config
    }
}
