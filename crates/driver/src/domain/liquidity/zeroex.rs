use {
    crate::domain::eth,
    anyhow::anyhow,
    contracts::IZeroEx,
    ethcontract::Bytes,
    model::order::OrderKind,
    primitive_types::U256,
    shared::zeroex_api::Order,
    solver::liquidity::{zeroex, LimitOrderId},
};

#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub id: LimitOrderId,
    pub order: Order,
    pub full_execution_amount: U256,
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
            order: handler.order,
            full_execution_amount,
            zeroex: handler.zeroex,
        })
    }

    pub fn to_interaction(&self) -> anyhow::Result<eth::Interaction> {
        let method = self.zeroex.fill_or_kill_limit_order(
            (
                self.order.maker_token,
                self.order.taker_token,
                self.order.maker_amount,
                self.order.taker_amount,
                self.order.taker_token_fee_amount,
                self.order.maker,
                self.order.taker,
                self.order.sender,
                self.order.fee_recipient,
                Bytes(self.order.pool.0),
                self.order.expiry,
                self.order.salt,
            ),
            (
                self.order.signature.signature_type,
                self.order.signature.v,
                Bytes(self.order.signature.r.0),
                Bytes(self.order.signature.s.0),
            ),
            self.full_execution_amount.as_u128(),
        );
        let calldata = method.tx.data.ok_or(anyhow!("no calldata"))?;

        Ok(eth::Interaction {
            target: self.zeroex.address().into(),
            value: 0.into(),
            call_data: calldata.0.into(),
        })
    }
}
