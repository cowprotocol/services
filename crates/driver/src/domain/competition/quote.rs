use {
    crate::{
        domain::{competition, eth},
        infra::solver::{self, Solver},
        util,
    },
    itertools::Itertools,
};

#[derive(Debug)]
pub struct Config {
    pub optimal_timeout: std::time::Duration,
    pub fast_timeout: std::time::Duration,
}

/// A quot describing the expected price of an order.
#[derive(Debug)]
pub struct Quote {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
}

/// An order which needs to be quoted.
#[derive(Debug)]
pub struct Order {
    pub sell_token: eth::TokenAddress,
    pub buy_token: eth::TokenAddress,
    pub amount: Amount,
    pub valid_to: u32,
    /// See [`crate::domain::competition::order::Partial`].
    pub partial: bool,
    pub quality: Quality,
    pub gas_price: eth::EffectiveGasPrice,
}

#[derive(Debug, Clone, Copy)]
pub enum Amount {
    Sell(eth::U256),
    Buy(eth::U256),
}

impl Order {
    /// The asset being bought. Zero if this is a sell.
    pub fn buy(&self) -> eth::Asset {
        match self.amount {
            Amount::Sell(..) => eth::Asset {
                amount: Default::default(),
                token: self.buy_token,
            },
            Amount::Buy(amount) => eth::Asset {
                amount,
                token: self.buy_token,
            },
        }
    }

    /// The asset being sold. Zero if this is a buy.
    pub fn sell(&self) -> eth::Asset {
        match self.amount {
            Amount::Sell(amount) => eth::Asset {
                amount,
                token: self.sell_token,
            },
            Amount::Buy(..) => eth::Asset {
                amount: Default::default(),
                token: self.sell_token,
            },
        }
    }

    pub fn side(&self) -> competition::order::Side {
        match self.amount {
            Amount::Sell(..) => competition::order::Side::Sell,
            Amount::Buy(..) => competition::order::Side::Buy,
        }
    }
}

/// Quality of the quote to be generated. This value determines the time
/// allocated for the solver to generate the solution.
#[derive(Debug, Clone, Copy)]
pub enum Quality {
    Fast,
    Optimal,
}

impl Order {
    /// Generate a quote for this order. This calls `/solve` on the solver with
    /// a "fake" auction which contains a single order, and then determines
    /// the quote for the order based on the solution that the solver
    /// returns.
    pub async fn quote(&self, solver: &Solver, config: &Config) -> Result<Quote, Error> {
        let solution = solver
            .solve(&self.fake_auction(), self.deadline(config))
            .await?;
        let fulfillment = solution
            .trades
            .into_iter()
            .filter_map(|trade| match trade {
                competition::solution::Trade::Fulfillment(fulfillment) => Some(fulfillment),
                competition::solution::Trade::Jit(..) => None,
            })
            .exactly_one()
            .map_err(|_| Error::QuotingFailed)?;
        Ok(fulfillment.into())
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
                side: self.side(),
                fee: Default::default(),
                kind: competition::order::Kind::Market,
                app_data: Default::default(),
                partial: if self.partial {
                    competition::order::Partial::Yes {
                        executed: Default::default(),
                    }
                } else {
                    competition::order::Partial::No
                },
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

    fn deadline(&self, config: &Config) -> competition::SolverTimeout {
        match self.quality {
            Quality::Fast => config.fast_timeout.into(),
            Quality::Optimal => config.optimal_timeout.into(),
        }
    }
}

impl From<competition::solution::trade::Fulfillment> for Quote {
    fn from(value: competition::solution::trade::Fulfillment) -> Self {
        match value.order.side {
            competition::order::Side::Buy => Self {
                sell: value.order.sell,
                buy: value.executed.to_asset(&value.order),
            },
            competition::order::Side::Sell => Self {
                sell: value.executed.to_asset(&value.order),
                buy: value.order.buy,
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
