use {
    crate::{
        boundary,
        domain::{eth, liquidity},
    },
    anyhow::anyhow,
    contracts::IZeroEx,
    model::order::OrderKind,
    primitive_types::{H160, U256},
    shared::zeroex_api::Order,
    solver::{
        interactions::allowances::Allowances,
        liquidity::{zeroex, Exchange, LimitOrderId},
    },
    std::sync::Arc,
};

/// A signed 0x Protocol Limit Order [^1].
///
/// [^1]: <https://0x.org/docs/0x-limit-orders/docs/introduction>
#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub id: LimitOrderId,
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub scoring_fee: U256,
    pub exchange: Exchange,
    pub order: Order,
    pub zeroex: IZeroEx,
    pub allowances: Arc<Allowances>,
}

impl LimitOrder {
    pub fn new(limit_order: solver::liquidity::LimitOrder) -> anyhow::Result<Self> {
        let handler = limit_order
            .settlement_handling
            .as_any()
            .downcast_ref::<zeroex::OrderSettlementHandler>()
            .ok_or(anyhow!("not a zeroex::OrderSettlementHandler"))?
            .clone();

        Ok(Self {
            id: limit_order.id,
            sell_token: limit_order.sell_token,
            buy_token: limit_order.buy_token,
            sell_amount: limit_order.sell_amount,
            buy_amount: limit_order.buy_amount,
            kind: limit_order.kind,
            partially_fillable: limit_order.partially_fillable,
            scoring_fee: limit_order.scoring_fee,
            exchange: limit_order.exchange,
            order: handler.order,
            zeroex: handler.zeroex,
            allowances: handler.allowances,
        })
    }

    pub fn full_execution_amount(&self) -> U256 {
        match self.kind {
            OrderKind::Sell => self.sell_amount,
            OrderKind::Buy => self.buy_amount,
        }
    }

    pub fn swap(&self) -> Result<eth::Interaction, liquidity::InvalidSwap> {
        boundary::liquidity::zeroex::to_interaction(self).map_err(|_| liquidity::InvalidSwap)
    }
}
