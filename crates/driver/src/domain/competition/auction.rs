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
#[derive(Debug, Clone)]
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

    pub fn for_solver(&self, now: time::Now) -> Result<SolverDeadline, DeadlineExceeded> {
        let deadline = self.0 - Self::solver_time_buffer();
        if deadline <= now.now() {
            Err(DeadlineExceeded)
        } else {
            Ok(SolverDeadline(deadline))
        }
    }

    pub fn solver_time_buffer() -> chrono::Duration {
        chrono::Duration::seconds(1)
    }
}

/// The time limit passed to the solver. The solvers are given a time limit
/// that's slightly less than the actual auction [`Deadline`]. The reason for
/// this is to allow the solver to use the full deadline to search for the
/// most optimal solution, but still ensure there is time left for the
/// driver to forward the results back to the protocol or do some other
/// necessary work.
#[derive(Debug, Clone, Copy)]
pub struct SolverDeadline(chrono::DateTime<chrono::Utc>);

impl From<SolverDeadline> for chrono::DateTime<chrono::Utc> {
    fn from(value: SolverDeadline) -> Self {
        value.0
    }
}

impl SolverDeadline {
    pub fn timeout(&self, now: time::Now) -> Result<std::time::Duration, DeadlineExceeded> {
        let timeout = self.0 - now.now();
        if timeout <= chrono::Duration::zero() {
            Err(DeadlineExceeded)
        } else {
            Ok(timeout.to_std().expect("already checked non-negative"))
        }
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
