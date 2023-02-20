use crate::{domain::dex::slippage, infra::dex};

pub mod file;

pub struct BalancerConfig {
    pub sor: dex::balancer::Config,
    pub slippage: slippage::Limits,
}
