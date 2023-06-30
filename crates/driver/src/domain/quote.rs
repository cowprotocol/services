use {
    crate::{
        boundary,
        domain::{
            competition::{self, order, solution},
            eth,
            liquidity,
        },
        infra::{
            self,
            blockchain::{self, Ethereum},
            solver::{self, Solver},
        },
        util::{self, conv},
    },
    std::{collections::HashSet, iter},
};

/// A quote describing the expected outcome of an order.
#[derive(Debug)]
pub struct Quote {
    /// The amount that can be bought if this was a sell order, or sold if this
    /// was a buy order.
    pub amount: eth::U256,
    pub interactions: Vec<eth::Interaction>,
    pub solver: eth::Address,
}

impl Quote {
    fn new(eth: &Ethereum, order: &Order, solution: competition::Solution) -> Result<Self, Error> {
        let sell_price = solution
            .price(order.tokens.sell)
            .ok_or(QuotingFailed::ClearingSellMissing)?;
        let buy_price = solution
            .price(order.tokens.buy)
            .ok_or(QuotingFailed::ClearingBuyMissing)?;
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
            interactions: boundary::quote::encode_interactions(eth, &solution.interactions)?,
            solver: solution.solver.address(),
        })
    }
}

/// An order which needs to be quoted.
#[derive(Debug)]
pub struct Order {
    pub tokens: Tokens,
    pub amount: order::TargetAmount,
    pub side: order::Side,
    pub deadline: Deadline,
}

impl Order {
    /// Generate a quote for this order. This calls `/solve` on the solver with
    /// a "fake" auction which contains a single order, and then determines
    /// the quote for the order based on the solution that the solver
    /// returns.
    pub async fn quote(
        &self,
        eth: &Ethereum,
        solver: &Solver,
        liquidity: &infra::liquidity::Fetcher,
    ) -> Result<Quote, Error> {
        let liquidity = liquidity.fetch(&self.liquidity_pairs()).await;
        let gas_price = eth.gas_price().await?;
        let timeout = self.deadline.timeout()?;
        let solutions = solver
            .solve(&self.fake_auction(gas_price), &liquidity, timeout)
            .await?;
        Quote::new(
            eth,
            self,
            // TODO(#1468): choose the best solution in the future, but for now just pick the
            // first solution
            solutions
                .into_iter()
                .next()
                .ok_or(QuotingFailed::NoSolutions)?,
        )
    }

    fn fake_auction(&self, gas_price: eth::GasPrice) -> competition::Auction {
        competition::Auction {
            id: None,
            tokens: Default::default(),
            orders: vec![competition::Order {
                uid: Default::default(),
                receiver: None,
                valid_to: util::Timestamp::MAX,
                buy: self.buy(),
                sell: self.sell(),
                side: self.side,
                fee: Default::default(),
                kind: competition::order::Kind::Market,
                app_data: Default::default(),
                partial: competition::order::Partial::No,
                // TODO add actual pre- and post-interactions (#1491)
                pre_interactions: Default::default(),
                post_interactions: Default::default(),
                sell_token_balance: competition::order::SellTokenBalance::Erc20,
                buy_token_balance: competition::order::BuyTokenBalance::Erc20,
                signature: competition::order::Signature {
                    scheme: competition::order::signature::Scheme::Eip1271,
                    data: Default::default(),
                    signer: Default::default(),
                },
            }],
            gas_price: gas_price.effective().into(),
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

    /// The asset being sold, or a very large value if this is a buy, to
    /// facilitate surplus.
    fn sell(&self) -> eth::Asset {
        match self.side {
            order::Side::Sell => eth::Asset {
                amount: self.amount.into(),
                token: self.tokens.sell,
            },
            // Note that we intentionally do not use [`eth::U256::max_value()`]
            // as an order with this would cause overflows with the smart
            // contract, so buy orders requiring excessively large sell amounts
            // would not work anyway.
            order::Side::Buy => eth::Asset {
                amount: eth::U256::one() << 192,
                token: self.tokens.sell,
            },
        }
    }

    /// Returns the token pairs to fetch liquidity for.
    fn liquidity_pairs(&self) -> HashSet<liquidity::TokenPair> {
        let pair = liquidity::TokenPair::new(self.tokens.sell(), self.tokens.buy())
            .expect("sell != buy by construction");
        iter::once(pair).collect()
    }
}

/// The deadline for computing a quote for an order.
#[derive(Clone, Copy, Debug, Default)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl Deadline {
    /// Computes the timeout for solving an auction.
    pub fn timeout(self) -> Result<solution::SolverTimeout, DeadlineExceeded> {
        solution::SolverTimeout::new(self.into(), Self::time_buffer()).ok_or(DeadlineExceeded)
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
    #[error(transparent)]
    QuotingFailed(#[from] QuotingFailed),
    #[error("{0:?}")]
    DeadlineExceeded(#[from] DeadlineExceeded),
    /// Encountered an unexpected error reading blockchain data.
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum QuotingFailed {
    #[error("missing sell price in solution clearing prices")]
    ClearingSellMissing,
    #[error("missing buy price in solution clearing prices")]
    ClearingBuyMissing,
    #[error("solver returned no solutions")]
    NoSolutions,
}

#[derive(Debug, thiserror::Error)]
#[error("the quoting deadline has been exceeded")]
pub struct DeadlineExceeded;

#[derive(Debug, thiserror::Error)]
#[error("the quoted tokens are the same")]
pub struct SameTokens;
