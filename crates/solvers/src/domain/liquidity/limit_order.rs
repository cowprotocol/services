use {crate::domain::eth, ethereum_types::U256};

/// A 0x-like foreign limit order.
#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub maker: eth::Asset,
    pub taker: eth::Asset,
    pub fee: TakerAmount,
}

/// An amount denominated in the taker token of a [`LimitOrder`].
#[derive(Debug, Clone, Copy)]
pub struct TakerAmount(pub U256);
