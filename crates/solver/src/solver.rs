use {
    anyhow::anyhow,
    ethcontract::{errors::ExecutionError, U256},
    std::{fmt::Debug, str::FromStr},
};

mod baseline_solver;
pub mod naive_solver;

#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("web3 error: {0:?}")]
    Web3(#[from] ExecutionError),
    #[error("insufficient balance: needs {needs} has {has}")]
    InsufficientBalance { needs: U256, has: U256 },
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum SolverType {
    None,
    Naive,
    Baseline,
    OneInch,
    Paraswap,
    ZeroEx,
    Quasimodo,
    BalancerSor,
}

// Wrapper type for AWS ARN identifiers
#[derive(Debug, Clone)]
pub struct Arn(pub String);

impl FromStr for Arn {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        // Could be more strict here, but this should suffice to catch unintended
        // configuration mistakes
        if s.starts_with("arn:aws:kms:") {
            Ok(Self(s.to_string()))
        } else {
            Err(anyhow!("Invalid ARN identifier: {}", s))
        }
    }
}
