use {
    crate::domain::{eth, liquidity, order},
    ethereum_types::U256,
    std::collections::HashMap,
};

/// The auction that the solvers need to find solutions to.
#[derive(Debug)]
pub struct Auction {
    pub id: Option<Id>,
    pub tokens: HashMap<eth::TokenAddress, Token>,
    pub orders: Vec<order::Order>,
    pub liquidity: Vec<liquidity::Liquidity>,
    pub gas_price: GasPrice,
    pub deadline: chrono::DateTime<chrono::Utc>,
}

/// The ID of an auction.
#[derive(Clone, Debug)]
pub struct Id(pub String);

#[derive(Debug)]
pub struct Token {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    pub reference_price: Option<Price>,
    pub available_balance: U256,
    pub trusted: bool,
}

/// The price of a token in wei. This represents how much wei is needed to buy
/// 10**18 of another token.
#[derive(Clone, Copy, Debug)]
pub struct Price(pub eth::Ether);

/// The estimated effective gas price that will likely be used for executing the
/// settlement transaction.
#[derive(Clone, Copy, Debug)]
pub struct GasPrice(pub eth::Ether);
