use {
    crate::domain::eth,
    anyhow::anyhow,
    contracts::IZeroEx,
    ethcontract::Bytes,
    primitive_types::{H160, H256, U256},
};

#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub order: Order,
    pub full_execution_amount: U256,
    pub zeroex: IZeroEx,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub maker: H160,
    pub taker: H160,
    pub sender: H160,
    pub maker_token: H160,
    pub taker_token: H160,
    pub maker_amount: u128,
    pub taker_amount: u128,
    pub taker_token_fee_amount: u128,
    pub fee_recipient: H160,
    pub pool: H256,
    pub expiry: u64,
    pub salt: U256,
    pub signature_type: u8,
    pub signature_r: H256,
    pub signature_s: H256,
    pub signature_v: u8,
}

impl LimitOrder {
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
                self.order.signature_type,
                self.order.signature_v,
                Bytes(self.order.signature_r.0),
                Bytes(self.order.signature_s.0),
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
