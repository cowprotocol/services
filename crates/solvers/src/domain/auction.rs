use {
    crate::domain::{eth, liquidity, order},
    ethereum_types::U256,
    std::collections::HashMap,
};

/// The auction that the solvers need to find solutions to.
#[derive(Debug)]
pub struct Auction {
    /// [`None`] if the auction applies to a quote.
    pub id: Option<Id>,
    pub tokens: Tokens,
    pub orders: Vec<order::Order>,
    pub liquidity: Vec<liquidity::Liquidity>,
    pub gas_price: GasPrice,
    pub deadline: chrono::DateTime<chrono::Utc>,
}

/// Information about tokens used in the auction.
#[derive(Debug)]
pub struct Tokens(pub HashMap<eth::TokenAddress, Token>);

impl Tokens {
    pub fn get(&self, token: &eth::TokenAddress) -> Option<&Token> {
        self.0.get(token)
    }

    pub fn reference_price(&self, token: &eth::TokenAddress) -> Option<Price> {
        self.get(token)?.reference_price
    }

    pub fn decimals(&self, token: &eth::TokenAddress) -> Option<u8> {
        self.get(token)?.decimals
    }
}

/// The ID of an auction.
#[derive(Clone, Debug)]
pub struct Id(pub i64);

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

impl Price {
    /// The base Ether amount for pricing.
    const BASE: u128 = 10_u128.pow(18);

    /// Computes an amount equivalent in value to the specified [`eth::Ether`]
    /// at the given price.
    pub fn ether_value(&self, eth: eth::Ether) -> Option<U256> {
        eth.0.checked_mul(Self::BASE.into())?.checked_div(self.0 .0)
    }
}

/// The estimated effective gas price that will likely be used for executing the
/// settlement transaction.
#[derive(Clone, Copy, Debug)]
pub struct GasPrice(pub eth::Ether);
