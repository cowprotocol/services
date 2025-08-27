use {
    contracts::alloy::IZeroex,
    ethcontract::Bytes,
    ethrpc::alloy::conversions::{ToAlloy, ToLegacy},
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
    pub zeroex: Arc<IZeroex::Instance>,
}

impl Interaction for ZeroExInteraction {
    fn encode(&self) -> EncodedInteraction {
        let method = self.zeroex.fillOrKillLimitOrder(
            IZeroex::LibNativeOrder::LimitOrder {
                makerToken: self.order.maker_token.to_alloy(),
                takerToken: self.order.taker_token.to_alloy(),
                makerAmount: self.order.maker_amount,
                takerAmount: self.order.taker_amount,
                takerTokenFeeAmount: self.order.taker_token_fee_amount,
                maker: self.order.maker.to_alloy(),
                taker: self.order.taker.to_alloy(),
                sender: self.order.sender.to_alloy(),
                feeRecipient: self.order.fee_recipient.to_alloy(),
                pool: self.order.pool.to_alloy(),
                expiry: self.order.expiry,
                salt: self.order.salt.to_alloy(),
            },
            IZeroex::LibSignature::Signature {
                signatureType: self.order.signature.signature_type,
                v: self.order.signature.v,
                r: self.order.signature.r.to_alloy(),
                s: self.order.signature.s.to_alloy(),
            },
            self.taker_token_fill_amount,
        );
        let calldata = method.calldata();
        (
            self.zeroex.address().to_legacy(),
            0.into(),
            Bytes(calldata.to_vec()),
        )
    }
}
