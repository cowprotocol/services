use {
    crate::domain::{eth, liquidity},
    alloy::primitives::{Address, B256, U256},
    anyhow::anyhow,
    contracts::IZeroex,
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
    // todo: remove Arc
    pub zeroex: Arc<IZeroex::Instance>,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub maker: Address,
    pub taker: Address,
    pub sender: Address,
    pub maker_token: Address,
    pub taker_token: Address,
    pub amounts: Amounts,
    pub taker_token_fee_amount: u128,
    pub fee_recipient: Address,
    pub pool: B256,
    pub expiry: u64,
    pub salt: U256,
    pub signature: ZeroExSignature,
}

#[derive(Clone, Debug)]
pub struct ZeroExSignature {
    pub r: B256,
    pub s: B256,
    pub v: u8,
    pub signature_type: u8,
}

impl LimitOrder {
    pub fn to_interaction(&self, input: &liquidity::MaxInput) -> anyhow::Result<eth::Interaction> {
        let method = self.zeroex.fillOrKillLimitOrder(
            IZeroex::LibNativeOrder::LimitOrder {
                makerToken: self.order.maker_token,
                takerToken: self.order.taker_token,
                makerAmount: self.order.amounts.maker,
                takerAmount: self.order.amounts.taker,
                takerTokenFeeAmount: self.order.taker_token_fee_amount,
                maker: self.order.maker,
                taker: self.order.taker,
                sender: self.order.sender,
                feeRecipient: self.order.fee_recipient,
                pool: self.order.pool,
                expiry: self.order.expiry,
                salt: self.order.salt,
            },
            IZeroex::LibSignature::Signature {
                signatureType: self.order.signature.signature_type,
                v: self.order.signature.v,
                r: self.order.signature.r,
                s: self.order.signature.s,
            },
            input
                .0
                .amount
                .0
                .try_into()
                .map_err(|_| anyhow!("executed amount does not fit into u128"))?,
        );
        let calldata = method.calldata();

        Ok(eth::Interaction {
            target: *self.zeroex.address(),
            value: 0.into(),
            call_data: calldata.to_vec().into(),
        })
    }
}
