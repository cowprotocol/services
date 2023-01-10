use crate::{
    domain::{competition, eth},
    infra::solver::{self, Solver},
    util,
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
    /// Turn the amount into a buy asset. Return zero if this is a sell.
    pub fn buy_asset(&self) -> eth::Asset {
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

    /// Turn the amount into a sell asset. Return zero if this is a buy.
    pub fn sell_asset(&self) -> eth::Asset {
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
    /// Generate a quote for this order. TODO Write a description of how the
    /// quoting process works.
    pub async fn quote(&self, solver: &Solver, config: &Config) -> Result<Quote, solver::Error> {
        // TODO Move this into a fake_auction method
        let auction = competition::Auction {
            id: None,
            tokens: Default::default(),
            orders: vec![competition::Order {
                uid: Default::default(),
                receiver: None,
                valid_to: util::Timestamp::MAX,
                sell: self.sell_asset(),
                buy: self.buy_asset(),
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
            // TODO Deadline wasn't designed correctly
            deadline: match self.quality {
                Quality::Fast => todo!(),
                Quality::Optimal => todo!(),
            },
        };
        // TODO So immediately this is problematic because it forces the solver to
        // return a SINGLE fulfillment, and no trades, seems like this isn't
        // very good
        let solution = solver.solve(&auction).await?;
        // TODO Check returned trades, error otherwise, don't panic anywhere
        // TODO Possibly just filter out the fulfillments and expect there to be exactly
        // one, i.e. ignore the JIT trades
        let trade = solution.trades.get(0).unwrap();
        Ok(Quote {
            sell: match trade {
                competition::solution::Trade::Fulfillment(fulfillment) => {
                    fulfillment.executed.to_asset(&fulfillment.order)
                }
                competition::solution::Trade::Jit(..) => panic!("should error instead"),
            },
            buy: todo!(),
        })
    }
}
