use crate::domain::eth;

pub mod file;

pub struct Config {
    pub weth: eth::WethAddress,
    pub base_tokens: Vec<eth::TokenAddress>,
    pub max_hops: usize,
    pub max_partial_attempts: usize,
    pub risk_parameters: super::RiskParameters,
}
