use crate::{domain::dex::slippage, infra::dex};

pub mod file;

pub struct Config {
    pub zeroex: dex::zeroex::Config,
    pub slippage: slippage::Limits,
}
