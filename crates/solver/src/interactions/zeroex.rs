use {
    contracts::IZeroex,
    shared::{
        interaction::{EncodedInteraction, Interaction},
        zeroex_api::Order,
    },
    std::sync::Arc,
};

#[derive(Clone, Debug)]
pub struct ZeroExInteraction {
    pub order: Order,
    pub taker_token_fill_amount: u128,
    // todo: remove Arc
    pub zeroex: Arc<IZeroex::Instance>,
}

impl Interaction for ZeroExInteraction {
    fn encode(&self) -> EncodedInteraction {
        let method = self.zeroex.fillOrKillLimitOrder(
            IZeroex::LibNativeOrder::LimitOrder {
                makerToken: self.order.maker_token,
                takerToken: self.order.taker_token,
                makerAmount: self.order.maker_amount,
                takerAmount: self.order.taker_amount,
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
            self.taker_token_fill_amount,
        );
        let calldata = method.calldata();
        (
            *self.zeroex.address(),
            alloy::primitives::U256::ZERO,
            calldata.to_vec().into(),
        )
    }
}
