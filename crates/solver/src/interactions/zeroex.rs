use {
    contracts::alloy::IZeroex,
    ethcontract::Bytes,
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
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
                makerToken: self.order.maker_token.into_alloy(),
                takerToken: self.order.taker_token.into_alloy(),
                makerAmount: self.order.maker_amount,
                takerAmount: self.order.taker_amount,
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
            self.taker_token_fill_amount,
        );
        let calldata = method.calldata();
        (
            self.zeroex.address().into_legacy(),
            0.into(),
            Bytes(calldata.to_vec()),
        )
    }
}
