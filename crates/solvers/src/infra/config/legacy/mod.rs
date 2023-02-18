use {crate::domain::eth, reqwest::Url};

pub mod file;

pub struct LegacyConfig {
    pub weth: eth::WethAddress,
    pub solver_name: String,
    pub chain_id: eth::ChainId,
    pub solve_endpoint: Url,
}
