use {
    eth_domain_types::{Address, ContractAddress, TokenAddress, TokenAmount},
    solvers_dto::auction::FlashloanHint,
};

#[derive(Debug, Clone)]
pub struct Flashloan {
    pub liquidity_provider: ContractAddress,
    pub protocol_adapter: ContractAddress,
    pub receiver: Address,
    pub token: TokenAddress,
    pub amount: TokenAmount,
}

impl From<&solvers_dto::solution::Flashloan> for Flashloan {
    fn from(value: &solvers_dto::solution::Flashloan) -> Self {
        Self {
            liquidity_provider: value.liquidity_provider.into(),
            protocol_adapter: value.protocol_adapter.into(),
            receiver: value.receiver,
            token: value.token.into(),
            amount: value.amount.into(),
        }
    }
}

#[expect(clippy::from_over_into)]
impl Into<solvers_dto::solution::Flashloan> for &Flashloan {
    fn into(self) -> solvers_dto::solution::Flashloan {
        solvers_dto::solution::Flashloan {
            liquidity_provider: self.liquidity_provider.into(),
            protocol_adapter: self.protocol_adapter.into(),
            receiver: self.receiver,
            token: self.token.0.into(),
            amount: self.amount.into(),
        }
    }
}

#[expect(clippy::from_over_into)]
impl Into<FlashloanHint> for &Flashloan {
    fn into(self) -> FlashloanHint {
        FlashloanHint {
            liquidity_provider: self.liquidity_provider.into(),
            protocol_adapter: self.protocol_adapter.into(),
            receiver: self.receiver,
            token: self.token.0.into(),
            amount: self.amount.into(),
        }
    }
}
