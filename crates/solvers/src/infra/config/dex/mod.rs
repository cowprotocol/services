pub mod balancer;
mod file;
pub mod oneinch;
pub mod zeroex;

use crate::domain::{dex::slippage, eth};

pub struct Config {
    pub slippage: slippage::Limits,
    pub smallest_partial_fill: eth::Ether,
}
