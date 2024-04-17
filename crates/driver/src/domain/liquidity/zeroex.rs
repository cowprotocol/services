use {
    crate::domain::{eth, liquidity},
    anyhow::anyhow,
    contracts::IZeroEx,
    ethcontract::Bytes,
    primitive_types::{H160, H256, U256},
    std::sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Amounts {
    pub maker: u128,
    pub taker: u128,
}

#[derive(Clone, Debug)]
pub struct LimitOrder {
    pub order: Order,
    /// Scaled amounts according to how much of the partially fillable amounts
    /// were already used in the order.
    pub fillable: Amounts,
    pub zeroex: Arc<IZeroEx>,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub maker: H160,
    pub taker: H160,
    pub sender: H160,
    pub maker_token: H160,
    pub taker_token: H160,
    pub amounts: Amounts,
    pub taker_token_fee_amount: u128,
    pub fee_recipient: H160,
    pub pool: H256,
    pub expiry: u64,
    pub salt: U256,
    pub signature: ZeroExSignature,
}

#[derive(Clone, Debug)]
pub struct ZeroExSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
    pub signature_type: u8,
}

impl LimitOrder {
    pub fn to_interaction(&self, input: &liquidity::MaxInput) -> anyhow::Result<eth::Interaction> {
        let method = self.zeroex.fill_or_kill_limit_order(
            (
                self.order.maker_token,
                self.order.taker_token,
                self.order.amounts.maker,
                self.order.amounts.taker,
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
            input
                .0
                .amount
                .0
                .try_into()
                .map_err(|_| anyhow!("executed amount does not fit into u128"))?,
        );
        let calldata = method.tx.data.ok_or(anyhow!("no calldata"))?;

        Ok(eth::Interaction {
            target: self.zeroex.address().into(),
            value: 0.into(),
            call_data: calldata.0.into(),
        })
    }
}
