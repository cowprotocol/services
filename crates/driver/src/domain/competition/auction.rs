use {
    crate::domain::{
        competition::{self, solution},
        eth,
    },
    std::collections::HashMap,
    thiserror::Error,
};

/// An auction is a set of orders that can be solved. The solvers calculate
/// [`super::solution::Solution`]s by picking subsets of these orders and
/// solving them.
#[derive(Debug)]
pub struct Auction {
    /// [`None`] if this auction applies to a quote. See
    /// [`crate::domain::quote`].
    pub id: Option<Id>,
    pub orders: Orders,
    pub tokens: Tokens,
    pub gas_price: eth::GasPrice,
    pub deadline: Deadline,
}

/// The orders for an auction. The orders get sorted such that those which are
/// more likely to be fulfilled come before less likely orders.
#[derive(Debug)]
pub struct Orders(Vec<competition::Order>);

impl Orders {
    pub fn new(mut orders: Vec<competition::Order>, tokens: &Tokens) -> Self {
        // Sort orders such that most likely to be fulfilled come first.
        orders.sort_by(|left, right| {
            // Market orders are preferred over limit orders, as the expectation is that
            // they should be immediately fulfillable. Liquidity orders come last, as they
            // are the most niche and rarely used.
            let left_kind = match left.kind {
                competition::order::Kind::Market => 2,
                competition::order::Kind::Limit { .. } => 1,
                competition::order::Kind::Liquidity => 0,
            };
            let right_kind = match right.kind {
                competition::order::Kind::Market => 2,
                competition::order::Kind::Limit { .. } => 1,
                competition::order::Kind::Liquidity => 0,
            };
            match left_kind.cmp(&right_kind) {
                std::cmp::Ordering::Equal => {
                    // If the orders are of the same kind, then sort by likelihood of fulfillment
                    // based on token prices.
                    left.likelihood(tokens).cmp(&right.likelihood(tokens))
                }
                other => other,
            }
        });
        orders.reverse();

        // TODO Filter out orders based on user balance

        Self(orders)
    }

    pub fn iter(&self) -> impl Iterator<Item = &competition::Order> {
        self.0.iter()
    }
}

/// The tokens that are used in an auction.
#[derive(Debug, Default)]
pub struct Tokens(HashMap<eth::TokenAddress, Token>);

impl Tokens {
    pub fn new(tokens: impl Iterator<Item = Token>) -> Self {
        Self(tokens.map(|token| (token.address, token)).collect())
    }

    pub fn get(&self, address: eth::TokenAddress) -> Token {
        self.0.get(&address).cloned().unwrap_or(Token {
            decimals: None,
            symbol: None,
            address,
            price: None,
            available_balance: Default::default(),
            trusted: false,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Token> {
        self.0.values()
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    pub address: eth::TokenAddress,
    pub price: Option<Price>,
    // TODO Set this field correctly, currently it isn't being passed into the driver.
    /// The balance of this token available in our settlement contract.
    pub available_balance: eth::U256,
    /// Is this token well-known and trusted by the protocol?
    pub trusted: bool,
}

/// The price of a token in wei. This represents how much wei is needed to buy
/// 10**18 of another token.
#[derive(Debug, Clone, Copy)]
pub struct Price(pub eth::Ether);

impl Price {
    /// Apply this price to some token amount, converting that token into ETH.
    pub fn apply(self, amount: eth::TokenAmount) -> eth::Ether {
        (amount.0 * self.0 .0).into()
    }
}

impl From<Price> for eth::U256 {
    fn from(value: Price) -> Self {
        value.0.into()
    }
}

impl From<eth::U256> for Price {
    fn from(value: eth::U256) -> Self {
        Self(value.into())
    }
}

/// Each auction has a deadline, limiting the maximum time that can be allocated
/// to solving the auction.
#[derive(Debug, Default, Clone, Copy)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl Deadline {
    /// Computes the timeout for solving an auction.
    pub fn timeout(self) -> Result<solution::SolverTimeout, solution::DeadlineExceeded> {
        solution::SolverTimeout::new(self.into(), Self::time_buffer())
    }

    pub fn time_buffer() -> chrono::Duration {
        chrono::Duration::seconds(1)
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Deadline {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self(value)
    }
}

impl From<Deadline> for chrono::DateTime<chrono::Utc> {
    fn from(value: Deadline) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Id(i64);

impl Id {
    pub fn to_be_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

impl TryFrom<i64> for Id {
    type Error = InvalidId;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value >= 0 {
            Ok(Self(value))
        } else {
            Err(InvalidId)
        }
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Error)]
#[error("the solution deadline has been exceeded")]
pub struct DeadlineExceeded;

#[derive(Debug, Error)]
#[error("invalid auction id")]
pub struct InvalidId;
