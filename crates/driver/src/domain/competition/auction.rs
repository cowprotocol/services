use {
    crate::{
        domain::{competition, eth, liquidity},
        infra::time,
    },
    std::{num::ParseIntError, str::FromStr},
    thiserror::Error,
};

/// An auction is a set of orders that can be solved. The solvers calculate
/// [`super::solution::Solution`]s by picking subsets of these orders and
/// solving them.
#[derive(Debug)]
pub struct Auction {
    // TODO Make this non-optional and simplify things
    /// [`None`] if the auction is used for quoting, [`Some`] if the auction is
    /// used for competition.
    pub id: Option<Id>,
    pub tokens: Vec<Token>,
    pub orders: Vec<competition::Order>,
    pub liquidity: Vec<liquidity::Liquidity>,
    pub gas_price: eth::EffectiveGasPrice,
    pub deadline: Deadline,
}

#[derive(Debug)]
pub struct Token {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    pub address: eth::TokenAddress,
    pub price: Option<competition::Price>,
    /// The balance of this token available in our settlement contract.
    pub available_balance: eth::U256,
    /// Is this token well-known and trusted by the protocol?
    pub trusted: bool,
}

/// Each auction has a deadline, limiting the maximum time that can be allocated
/// to solving the auction.
#[derive(Debug, Default, Clone, Copy)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl Deadline {
    pub fn new(
        deadline: chrono::DateTime<chrono::Utc>,
        now: time::Now,
    ) -> Result<Self, DeadlineExceeded> {
        if deadline <= now.now() {
            Err(DeadlineExceeded)
        } else {
            Ok(Self(deadline))
        }
    }
}

impl From<Deadline> for chrono::DateTime<chrono::Utc> {
    fn from(value: Deadline) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Id(pub u64);

impl From<u64> for Id {
    fn from(inner: u64) -> Self {
        Self(inner)
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Id {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FromStr::from_str(s).map(Self)
    }
}

#[derive(Debug, Error)]
#[error("the solution deadline has been exceeded")]
pub struct DeadlineExceeded;
