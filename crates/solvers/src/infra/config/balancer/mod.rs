use crate::{domain::dex::slippage, infra::dex};

pub mod file;

pub struct Config {
    pub sor: dex::balancer::Config,
    pub slippage: slippage::Limits,
}
