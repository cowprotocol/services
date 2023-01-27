use {
    crate::{
        domain::{
            competition::{self, order, solution},
            eth,
            liquidity,
        },
        infra::{
            self,
            solver::{self, Solver},
            time,
        },
        util::{self, conv},
    },
    std::{collections::HashSet, iter},
};

pub const FAKE_AUCTION_REWARD: f64 = 35.;

/// A quote describing the expected outcome of an order.
#[derive(Debug)]
pub struct Quote {
    /// The amount that can be bought if this was a sell order, or sold if this
    /// was a buy order.
    pub amount: eth::U256,
    pub interactions: Vec<competition::solution::Interaction>,
}

impl Quote {
    fn new(order: &Order, solution: competition::Solution) -> Result<Self, Error> {
        let sell_price = solution
            .prices
            .get(&order.tokens.sell)
            .ok_or(Error::QuotingFailed)?
            .to_owned();
        let buy_price = solution
            .prices
            .get(&order.tokens.buy)
            .ok_or(Error::QuotingFailed)?
            .to_owned();
        let amount = match order.side {
            order::Side::Sell => conv::u256::from_big_rational(
                &(conv::u256::to_big_rational(order.amount.into())
                    * conv::u256::to_big_rational(sell_price)
                    / conv::u256::to_big_rational(buy_price)),
            ),
            order::Side::Buy => conv::u256::from_big_rational(
                &(conv::u256::to_big_rational(order.amount.into())
                    * conv::u256::to_big_rational(buy_price)
                    / conv::u256::to_big_rational(sell_price)),
            ),
        };
        Ok(Self {
            amount,
            interactions: solution.interactions,
        })
    }
}

/// An order which needs to be quoted.
#[derive(Debug)]
pub struct Order {
    pub tokens: Tokens,
    pub amount: order::TargetAmount,
    pub side: order::Side,
    pub gas_price: eth::EffectiveGasPrice,
    pub deadline: Deadline,
}

impl Order {
    /// Generate a quote for this order. This calls `/solve` on the solver with
    /// a "fake" auction which contains a single order, and then determines
    /// the quote for the order based on the solution that the solver
    /// returns.
    pub async fn quote(
        &self,
        solver: &Solver,
        liquidity: &infra::liquidity::Fetcher,
        now: time::Now,
    ) -> Result<Quote, Error> {
        let liquidity = liquidity.fetch(&self.liquidity_pairs()).await?;
        let timeout = self.deadline.timeout(now)?;
        let solution = solver
            .solve(&self.fake_auction(), &liquidity, timeout)
            .await?;
        Quote::new(self, solution)
    }

    fn fake_auction(&self) -> competition::Auction {
        competition::Auction {
            id: None,
            tokens: Default::default(),
            orders: vec![competition::Order {
                uid: Default::default(),
                receiver: None,
                valid_to: util::Timestamp::MAX,
                sell: self.sell(),
                buy: self.buy(),
                side: self.side,
                fee: Default::default(),
                kind: competition::order::Kind::Market,
                app_data: Default::default(),
                partial: competition::order::Partial::No,
                interactions: Default::default(),
                sell_token_balance: competition::order::SellTokenBalance::Erc20,
                buy_token_balance: competition::order::BuyTokenBalance::Erc20,
                signature: competition::order::Signature {
                    scheme: competition::order::signature::Scheme::Eip1271,
                    data: Default::default(),
                    signer: Default::default(),
                },
                reward: FAKE_AUCTION_REWARD,
            }],
            gas_price: self.gas_price,
            deadline: Default::default(),
        }
    }

    /// The asset being bought, or [`eth::U256::one`] if this is a sell, to
    /// facilitate surplus.
    fn buy(&self) -> eth::Asset {
        match self.side {
            order::Side::Sell => eth::Asset {
                amount: eth::U256::one(),
                token: self.tokens.buy,
            },
            order::Side::Buy => eth::Asset {
                amount: self.amount.into(),
                token: self.tokens.buy,
            },
        }
    }

    /// The asset being sold, or [`eth::U256::max_value`] if this is a buy, to
    /// facilitate surplus.
    fn sell(&self) -> eth::Asset {
        match self.side {
            order::Side::Sell => eth::Asset {
                amount: self.amount.into(),
                token: self.tokens.sell,
            },
            order::Side::Buy => eth::Asset {
                amount: eth::U256::max_value(),
                token: self.tokens.sell,
            },
        }
    }

    /// Returns the token pairs to fetch liquidity for.
    fn liquidity_pairs(&self) -> HashSet<liquidity::TokenPair> {
        let pair = liquidity::TokenPair::new(self.tokens.sell(), self.tokens.buy())
            .expect("sell != buy by construction");
        iter::once(pair).into_iter().collect()
    }
}

/// The deadline for computing a quote for an order.
#[derive(Clone, Copy, Debug, Default)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl Deadline {
    /// Computes the timeout for solving an auction.
    pub fn timeout(self, now: time::Now) -> Result<solution::SolverTimeout, DeadlineExceeded> {
        solution::SolverTimeout::new(self.into(), Self::time_buffer(), now).ok_or(DeadlineExceeded)
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

/// The sell and buy tokens to quote for. This type maintains the invariant that
/// the sell and buy tokens are distinct.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Tokens {
    sell: eth::TokenAddress,
    buy: eth::TokenAddress,
}

impl Tokens {
    /// Creates a new instance of [`Tokens`], verifying that the input buy and
    /// sell tokens are distinct.
    pub fn new(sell: eth::TokenAddress, buy: eth::TokenAddress) -> Result<Self, SameTokens> {
        if sell == buy {
            return Err(SameTokens);
        }
        Ok(Self { sell, buy })
    }

    pub fn sell(&self) -> eth::TokenAddress {
        self.sell
    }

    pub fn buy(&self) -> eth::TokenAddress {
        self.buy
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// This can happen e.g. if there's no available liquidity for the tokens
    /// which the user is trying to trade.
    #[error("solver was unable to generate a quote for this order")]
    QuotingFailed,
    #[error("{0:?}")]
    DeadlineExceeded(#[from] DeadlineExceeded),
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
    #[error("liquidity fetcher error: {0:?}")]
    Liquidity(#[from] infra::liquidity::fetcher::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("the quoting deadline has been exceeded")]
pub struct DeadlineExceeded;

#[derive(Debug, thiserror::Error)]
#[error("the quoted tokens are the same")]
pub struct SameTokens;
