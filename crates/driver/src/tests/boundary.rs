//! Similar to [`crate::boundary`], but for test code.

pub use model::{DomainSeparator, order::OrderUid};
use {
    crate::domain::competition,
    alloy::{
        primitives::{Address, U256},
        signers::local::PrivateKeySigner,
    },
};

/// Order data used for calculating the order UID and signing.
#[derive(Debug)]
pub struct Order {
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub valid_to: u32,
    pub receiver: Option<Address>,
    pub user_fee: U256,
    pub side: competition::order::Side,
    pub secret_key: PrivateKeySigner,
    pub domain_separator: DomainSeparator,
    pub owner: Address,
    pub partially_fillable: bool,
}

impl Order {
    pub fn uid(&self) -> OrderUid {
        self.build().data.uid(&self.domain_separator, self.owner)
    }

    pub fn signature(&self) -> Vec<u8> {
        self.build().signature.to_bytes()
    }

    fn build(&self) -> model::order::Order {
        model::order::OrderBuilder::default()
            .with_sell_token(self.sell_token)
            .with_buy_token(self.buy_token)
            .with_sell_amount(self.sell_amount)
            .with_buy_amount(self.buy_amount)
            .with_valid_to(self.valid_to)
            .with_fee_amount(self.user_fee)
            .with_receiver(self.receiver)
            .with_kind(match self.side {
                competition::order::Side::Buy => model::order::OrderKind::Buy,
                competition::order::Side::Sell => model::order::OrderKind::Sell,
            })
            .with_partially_fillable(self.partially_fillable)
            .sign_with(
                model::signature::EcdsaSigningScheme::Eip712,
                &self.domain_separator,
                &self.secret_key,
            )
            .build()
    }
}
