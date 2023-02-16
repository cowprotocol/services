use {crate::domain::eth, reqwest::Url};

pub mod file;

pub struct LegacyConfig {
    pub weth: eth::WethAddress,
    pub solver_name: String,
    pub chain_id: eth::ChainId,
    pub base_url: Url,
    pub max_nr_exec_orders: u32,
}
