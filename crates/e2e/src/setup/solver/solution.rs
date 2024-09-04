use {
    app_data::AppDataHash,
    ethcontract::common::abi::ethereum_types::Address,
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, OrderUid, SellTokenSource},
        signature::EcdsaSigningScheme,
        DomainSeparator,
    },
    solvers_dto::solution::{Asset, Kind},
    web3::signing::SecretKeyRef,
};

#[derive(Clone, Debug)]
pub struct JitOrder {
    pub owner: Address,
    pub sell: Asset,
    pub buy: Asset,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub valid_to: u32,
    pub app_data: AppDataHash,
    pub receiver: Address,
}

impl JitOrder {
    fn data(&self) -> OrderData {
        OrderData {
            sell_token: self.sell.token,
            buy_token: self.buy.token,
            receiver: self.receiver.into(),
            sell_amount: self.sell.amount,
            buy_amount: self.buy.amount,
            valid_to: self.valid_to,
            app_data: AppDataHash(self.app_data.0),
            fee_amount: 0.into(),
            kind: self.kind,
            partially_fillable: self.partially_fillable,
            sell_token_balance: Default::default(),
            buy_token_balance: Default::default(),
        }
    }

    pub fn sign(
        self,
        signing_scheme: EcdsaSigningScheme,
        domain: &DomainSeparator,
        key: SecretKeyRef,
    ) -> (solvers_dto::solution::JitOrder, OrderUid) {
        let data = self.data();
        let signature = model::signature::EcdsaSignature::sign(
            signing_scheme,
            domain,
            &data.hash_struct(),
            key,
        )
        .to_signature(signing_scheme);
        let order_uid = data.uid(
            domain,
            &signature
                .recover_owner(&signature.to_bytes(), domain, &data.hash_struct())
                .unwrap(),
        );
        let signature = match signature {
            model::signature::Signature::Eip712(signature) => signature.to_bytes().to_vec(),
            model::signature::Signature::EthSign(signature) => signature.to_bytes().to_vec(),
            model::signature::Signature::Eip1271(signature) => signature,
            model::signature::Signature::PreSign => panic!("Not supported PreSigned JIT orders"),
        };
        let order = solvers_dto::solution::JitOrder {
            sell_token: data.sell_token,
            buy_token: data.buy_token,
            receiver: data.receiver.unwrap_or_default(),
            sell_amount: data.sell_amount,
            buy_amount: data.buy_amount,
            valid_to: data.valid_to,
            app_data: data.app_data.0,
            kind: match data.kind {
                OrderKind::Buy => Kind::Buy,
                OrderKind::Sell => Kind::Sell,
            },
            sell_token_balance: match data.sell_token_balance {
                SellTokenSource::Erc20 => solvers_dto::solution::SellTokenBalance::Erc20,
                SellTokenSource::External => solvers_dto::solution::SellTokenBalance::External,
                SellTokenSource::Internal => solvers_dto::solution::SellTokenBalance::Internal,
            },
            buy_token_balance: match data.buy_token_balance {
                BuyTokenDestination::Erc20 => solvers_dto::solution::BuyTokenBalance::Erc20,
                BuyTokenDestination::Internal => solvers_dto::solution::BuyTokenBalance::Internal,
            },
            signing_scheme: match signing_scheme {
                EcdsaSigningScheme::Eip712 => solvers_dto::solution::SigningScheme::Eip712,
                EcdsaSigningScheme::EthSign => solvers_dto::solution::SigningScheme::EthSign,
            },
            signature,
        };
        (order, order_uid)
    }
}
