use {
    crate::{
        boundary,
        domain::{eth, liquidity},
    },
    anyhow::anyhow,
    contracts::IZeroEx,
    model::order::OrderKind,
    primitive_types::U256,
    shared::zeroex_api::Order,
    solver::liquidity::{zeroex, LimitOrderId},
};

/// A signed 0x Protocol Limit Order [^1].
///
/// [^1]: <https://0x.org/docs/0x-limit-orders/docs/introduction>
#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub id: LimitOrderId,
    pub full_execution_amount: U256,
    pub order: Order,
    pub zeroex: IZeroEx,
}

impl LimitOrder {
    pub fn new(limit_order: solver::liquidity::LimitOrder) -> anyhow::Result<Self> {
        let handler = limit_order
            .settlement_handling
            .as_any()
            .downcast_ref::<zeroex::OrderSettlementHandler>()
            .ok_or(anyhow!("not a zeroex::OrderSettlementHandler"))?
            .clone();

        let full_execution_amount = match limit_order.kind {
            OrderKind::Sell => limit_order.sell_amount,
            OrderKind::Buy => limit_order.buy_amount,
        };

        Ok(Self {
            id: limit_order.id,
            full_execution_amount,
            order: handler.order,
            zeroex: handler.zeroex,
        })
    }

    pub fn swap(&self) -> Result<eth::Interaction, liquidity::InvalidSwap> {
        boundary::liquidity::zeroex::to_interaction(self).map_err(|_| liquidity::InvalidSwap)
    }
}
