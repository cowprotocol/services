use model::OrderCreation;
use primitive_types::{H160, U256};
use std::collections::HashMap;

#[derive(Debug, Eq, PartialEq)]
pub struct Trade {
    pub order: OrderCreation,
    pub executed_amount: U256,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Interaction {
    // https://uniswap.org/docs/v2/smart-contracts/router02/#swapexacttokensfortokens
    AmmSwapExactTokensForTokens {
        amount_in: U256,
        amount_out_min: U256,
        token_in: H160,
        token_out: H160,
    },
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Settlement {
    pub clearing_prices: HashMap<H160, U256>,
    pub fee_factor: U256,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
    pub order_refunds: Vec<()>,
}
