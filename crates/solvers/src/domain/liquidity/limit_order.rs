use crate::domain::eth;
use ethereum_types::U256;

/// A 0x-like foreign limit order.
#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub maker: eth::Asset,
    pub taker: eth::Asset,
    pub fee: TakerAmount,
}

impl LimitOrder {
    /// Returns the fee amount as an asset.
    pub fn fee(&self) -> eth::Asset {
        eth::Asset {
            token: self.taker.token,
            amount: self.fee.0,
        }
    }
}

/// An amount denominated in the taker token of a [`LimitOrder`].
#[derive(Debug, Clone, Copy)]
pub struct TakerAmount(pub U256);
