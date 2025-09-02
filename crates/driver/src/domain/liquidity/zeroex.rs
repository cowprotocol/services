use {
    crate::domain::{eth, liquidity},
    anyhow::anyhow,
    contracts::alloy::IZeroex,
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
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
    // todo: remove Arc
    pub zeroex: Arc<IZeroex::Instance>,
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
        let method = self.zeroex.fillOrKillLimitOrder(
            IZeroex::LibNativeOrder::LimitOrder {
                makerToken: self.order.maker_token.into_alloy(),
                takerToken: self.order.taker_token.into_alloy(),
                makerAmount: self.order.amounts.maker,
                takerAmount: self.order.amounts.taker,
                takerTokenFeeAmount: self.order.taker_token_fee_amount,
                maker: self.order.maker.into_alloy(),
                taker: self.order.taker.into_alloy(),
                sender: self.order.sender.into_alloy(),
                feeRecipient: self.order.fee_recipient.into_alloy(),
                pool: self.order.pool.into_alloy(),
                expiry: self.order.expiry,
                salt: self.order.salt.into_alloy(),
            },
            IZeroex::LibSignature::Signature {
                signatureType: self.order.signature.signature_type,
                v: self.order.signature.v,
                r: self.order.signature.r.into_alloy(),
                s: self.order.signature.s.into_alloy(),
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
            target: self.zeroex.address().into_legacy().into(),
            value: 0.into(),
            call_data: calldata.to_vec().into(),
        })
    }
}
