use crate::{
    boundary,
    domain::{eth, liquidity},
};

/// A signed 0x Protocol Limit Order [^1].
///
/// [^1]: <https://0x.org/docs/0x-limit-orders/docs/introduction>
#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub inner: solver::liquidity::LimitOrder,
}

impl LimitOrder {
    pub fn new(limit_order: solver::liquidity::LimitOrder) -> Self {
        LimitOrder { inner: limit_order }
    }

    pub fn swap(
        &self,
        input: &liquidity::MaxInput,
        output: &liquidity::ExactOutput,
        receiver: &eth::Address,
    ) -> Result<eth::Interaction, liquidity::InvalidSwap> {
        boundary::liquidity::zeroex::to_interaction(self, input, output, receiver)
            .map_err(|_| liquidity::InvalidSwap)
    }
}
