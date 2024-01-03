use {
    crate::{boundary, domain::fee::Policy},
    primitive_types::{H160, U256},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub uid: boundary::OrderUid,
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub solver_fee: U256,
    pub user_fee: U256,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub receiver: Option<H160>,
    pub owner: H160,
    pub partially_fillable: bool,
    pub executed: U256,
    // Partially fillable orders should have their pre-interactions only executed
    // on the first fill.
    pub pre_interactions: Vec<Interaction>,
    pub post_interactions: Vec<Interaction>,
    pub sell_token_balance: boundary::SellTokenSource,
    pub buy_token_balance: boundary::BuyTokenDestination,
    pub class: Class,
    pub app_data: boundary::AppDataHash,
    pub signature: boundary::Signature,
    pub eth_flow: Option<boundary::EthflowData>,
    pub onchain_order: Option<boundary::OnchainOrderData>,
    pub fee_policies: Vec<Policy>,
}

impl Order {
    pub fn is_limit_order(&self) -> bool {
        matches!(self.class, Class::Limit)
    }

    /// For some orders the protocol doesn't precompute a fee. Instead solvers
    /// are supposed to compute a reasonable fee themselves.
    pub fn solver_determines_fee(&self) -> bool {
        self.is_limit_order()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrderKind {
    Buy,
    Sell,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    Market,
    Limit,
    Liquidity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interaction {
    pub target: H160,
    pub value: U256,
    pub call_data: Vec<u8>,
}
