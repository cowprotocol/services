use crate::{
    domain::{
        competition::{self, order, solution},
        eth,
    },
    infra::solver::{self, Solver},
    util::{self, conv},
};

#[derive(Debug)]
pub struct Config {
    pub timeout: solution::SolverTimeout,
}

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
            .get(&order.sell_token)
            .ok_or(Error::QuotingFailed)?
            .to_owned();
        let buy_price = solution
            .prices
            .get(&order.buy_token)
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
    pub sell_token: eth::TokenAddress,
    pub buy_token: eth::TokenAddress,
    pub amount: order::TargetAmount,
    pub side: order::Side,
    pub gas_price: eth::EffectiveGasPrice,
}

impl Order {
    /// Generate a quote for this order. This calls `/solve` on the solver with
    /// a "fake" auction which contains a single order, and then determines
    /// the quote for the order based on the solution that the solver
    /// returns.
    pub async fn quote(&self, solver: &Solver, config: &Config) -> Result<Quote, Error> {
        let solution = solver.solve(&self.fake_auction(), config.timeout).await?;
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
                reward: Default::default(),
            }],
            liquidity: Default::default(),
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
                token: self.buy_token,
            },
            order::Side::Buy => eth::Asset {
                amount: self.amount.into(),
                token: self.buy_token,
            },
        }
    }

    /// The asset being sold, or [`eth::U256::max_value`] if this is a buy, to
    /// facilitate surplus.
    fn sell(&self) -> eth::Asset {
        match self.side {
            order::Side::Sell => eth::Asset {
                amount: self.amount.into(),
                token: self.sell_token,
            },
            order::Side::Buy => eth::Asset {
                amount: eth::U256::max_value(),
                token: self.sell_token,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// This can happen e.g. if there's no available liquidity for the tokens
    /// which the user is trying to trade.
    #[error("solver was unable to generate a quote for this order")]
    QuotingFailed,
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
}
