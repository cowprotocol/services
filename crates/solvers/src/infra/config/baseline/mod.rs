use crate::domain::eth;

pub mod file;

pub struct BaselineConfig {
    pub weth: eth::WethAddress,
    pub base_tokens: Vec<eth::TokenAddress>,
    pub max_hops: usize,
}
