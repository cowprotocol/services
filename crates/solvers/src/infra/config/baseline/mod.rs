use crate::domain::eth;

pub mod file;

pub struct BaselineConfig {
    pub chain_id: eth::ChainId,
    pub weth: Option<eth::WethAddress>,
    pub base_tokens: Vec<eth::TokenAddress>,
    pub max_hops: usize,
}
