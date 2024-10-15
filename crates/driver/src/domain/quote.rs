use {
    super::competition::{auction, solution},
    crate::{
        boundary,
        domain::{
            competition::{self, order},
            eth,
            liquidity,
            time,
        },
        infra::{
            self,
            blockchain::{self, Ethereum},
            solver::{self, Solver},
        },
        util::{self, conv::u256::U256Ext},
    },
    anyhow::Context,
    chrono::Utc,
    num::CheckedDiv,
    std::{collections::HashSet, iter, ops::Mul},
};

/// A quote describing the expected outcome of an order.
#[derive(Debug)]
pub struct Quote {
    /// The amount that can be bought if this was a sell order, or sold if this
    /// was a buy order.
    pub amount: eth::U256,
    pub pre_interactions: Vec<eth::Interaction>,
    pub interactions: Vec<eth::Interaction>,
    pub solver: eth::Address,
    pub gas: Option<eth::Gas>,
    /// Which `tx.origin` is required to make the quote simulation pass.
    pub tx_origin: Option<eth::Address>,
    pub jit_orders: Vec<solution::trade::Jit>,
}

impl Quote {
    fn new(eth: &Ethereum, order: &Order, solution: competition::Solution) -> Result<Self, Error> {
        let sell_price = solution
            .clearing_price(order.tokens.sell)
            .ok_or(QuotingFailed::ClearingSellMissing)?
            .to_big_rational();
        let buy_price = solution
            .clearing_price(order.tokens.buy)
            .ok_or(QuotingFailed::ClearingBuyMissing)?
            .to_big_rational();
        let order_amount = order.amount.0.to_big_rational();

        let amount = match order.side {
            order::Side::Sell => order_amount
                .mul(sell_price)
                .checked_div(&buy_price)
                .context("div by zero: buy price")?,
            order::Side::Buy => order_amount
                .mul(&buy_price)
                .checked_div(&sell_price)
                .context("div by zero: sell price")?,
        };

        Ok(Self {
            amount: eth::U256::from_big_rational(&amount)?,
            pre_interactions: solution.pre_interactions().to_vec(),
            interactions: solution
                .interactions()
                .iter()
                .map(|i| encode::interaction(i, eth.contracts().settlement()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect(),
            solver: solution.solver().address(),
            gas: solution.gas(),
            tx_origin: *solution.solver().quote_tx_origin(),
            jit_orders: solution
                .trades()
                .iter()
                .filter_map(|trade| match trade {
                    solution::Trade::Jit(jit) => Some(jit.clone()),
                    _ => None,
                })
                .collect(),
        })
    }
}

/// An order which needs to be quoted.
#[derive(Debug)]
pub struct Order {
    pub tokens: Tokens,
    pub amount: order::TargetAmount,
    pub side: order::Side,
    pub deadline: time::Deadline,
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
        tokens: &infra::tokens::Fetcher,
    ) -> Result<Quote, Error> {
        let liquidity = match solver.liquidity() {
            solver::Liquidity::Fetch => {
                liquidity
                    .fetch(&self.liquidity_pairs(), infra::liquidity::AtBlock::Recent)
                    .await
            }
            solver::Liquidity::Skip => Default::default(),
        };

        let auction = self
            .fake_auction(eth, tokens, solver.quote_using_limit_orders())
            .await?;
        let solutions = solver.solve(&auction, &liquidity).await?;
        Quote::new(
            eth,
            self,
            // TODO(#1468): choose the best solution in the future, but for now just pick the
            // first solution
            solutions
                .into_iter()
                .find(|solution| !solution.is_empty(auction.surplus_capturing_jit_order_owners()))
                .ok_or(QuotingFailed::NoSolutions)?,
        )
    }

    async fn fake_auction(
        &self,
        eth: &Ethereum,
        tokens: &infra::tokens::Fetcher,
        quote_using_limit_orders: bool,
    ) -> Result<competition::Auction, Error> {
        let tokens = tokens.get(&[self.buy().token, self.sell().token]).await;

        let buy_token_metadata = tokens.get(&self.buy().token);
        let sell_token_metadata = tokens.get(&self.sell().token);

        competition::Auction::new(
            None,
            vec![competition::Order {
                uid: Default::default(),
                receiver: None,
                created: u32::try_from(Utc::now().timestamp())
                    .unwrap_or(u32::MIN)
                    .into(),
                valid_to: util::Timestamp::MAX,
                buy: self.buy(),
                sell: self.sell(),
                side: self.side,
                kind: if quote_using_limit_orders {
                    competition::order::Kind::Limit
                } else {
                    competition::order::Kind::Market
                },
                app_data: Default::default(),
                partial: competition::order::Partial::No,
                pre_interactions: Default::default(),
                post_interactions: Default::default(),
                sell_token_balance: competition::order::SellTokenBalance::Erc20,
                buy_token_balance: competition::order::BuyTokenBalance::Erc20,
                signature: competition::order::Signature {
                    scheme: competition::order::signature::Scheme::Eip1271,
                    data: Default::default(),
                    signer: Default::default(),
                },
                protocol_fees: Default::default(),
                quote: Default::default(),
            }],
            [
                auction::Token {
                    decimals: sell_token_metadata.and_then(|m| m.decimals),
                    symbol: sell_token_metadata.and_then(|m| m.symbol.clone()),
                    address: self.tokens.sell,
                    price: None,
                    available_balance: sell_token_metadata.map(|m| m.balance.0).unwrap_or_default(),
                    trusted: false,
                },
                auction::Token {
                    decimals: buy_token_metadata.and_then(|m| m.decimals),
                    symbol: buy_token_metadata.and_then(|m| m.symbol.clone()),
                    address: self.tokens.buy,
                    price: None,
                    available_balance: buy_token_metadata.map(|m| m.balance.0).unwrap_or_default(),
                    trusted: false,
                },
            ]
            .into_iter(),
            self.deadline,
            eth,
            HashSet::default(),
        )
        .await
        .map_err(|err| match err {
            auction::Error::InvalidTokens => panic!("fake auction with invalid tokens"),
            auction::Error::InvalidAmounts => panic!("fake auction with invalid amounts"),
            auction::Error::Blockchain(e) => e.into(),
        })
    }

    /// The asset being bought, or [`eth::U256::one`] if this is a sell, to
    /// facilitate surplus.
    fn buy(&self) -> eth::Asset {
        match self.side {
            order::Side::Sell => eth::Asset {
                amount: eth::U256::one().into(),
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
            // would not work anyway. Instead we use `2 ** 144`, the rationale
            // being that Uniswap V2 pool reserves are 112-bit integers. Noting
            // that `256 - 112 = 144`, this means that we can use it to trade a full
            // `type(uint112).max` without overflowing a `uint256` on the smart
            // contract level. Requiring to trade more than `type(uint112).max`
            // is unlikely and would not work with Uniswap V2 anyway.
            order::Side::Buy => eth::Asset {
                amount: (eth::U256::one() << 144).into(),
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

mod encode {
    use {
        crate::domain::{
            competition::solution,
            eth::{
                self,
                allowance::{Approval, Required},
            },
        },
        num::rational::Ratio,
    };

    const DEFAULT_QUOTE_SLIPPAGE_BPS: u32 = 100;

    pub(super) fn interaction(
        interaction: &solution::Interaction,
        settlement: &contracts::GPv2Settlement,
    ) -> Result<Vec<eth::Interaction>, solution::encoding::Error> {
        let slippage = solution::slippage::Parameters {
            relative: Ratio::new_raw(DEFAULT_QUOTE_SLIPPAGE_BPS.into(), 10_000.into()),
            max: None,
            min: None,
            prices: Default::default(),
        };

        let encoded = match interaction {
            solution::Interaction::Custom(interaction) => eth::Interaction {
                value: interaction.value,
                target: interaction.target.0.into(),
                call_data: interaction.call_data.clone(),
            },
            solution::Interaction::Liquidity(liquidity) => {
                solution::encoding::liquidity_interaction(liquidity, &slippage, settlement)?
            }
        };

        Ok(interaction
            .allowances()
            .iter()
            .flat_map(|Required(allowance)| {
                let approval = Approval(*allowance);
                // When encoding approvals for quotes, reset the allowance instead
                // of just setting it. This is required as some tokens only allow
                // you to approve a non-0 value if the allowance was 0 to begin
                // with, such as Tether USD.
                //
                // Alternatively, we could check existing allowances and only encode
                // the approvals if needed, but this would only result in small gas
                // optimizations which is mostly inconsequential for quotes and not
                // worth the performance hit.
                vec![
                    solution::encoding::approve(&approval.revoke().0),
                    solution::encoding::approve(&approval.max().0),
                ]
            })
            .chain(std::iter::once(encoded))
            .collect())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// This can happen e.g. if there's no available liquidity for the tokens
    /// which the user is trying to trade.
    #[error(transparent)]
    QuotingFailed(#[from] QuotingFailed),
    #[error("{0:?}")]
    DeadlineExceeded(#[from] time::DeadlineExceeded),
    /// Encountered an unexpected error reading blockchain data.
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
    #[error("encoding error: {0:?}")]
    Encoding(#[from] solution::encoding::Error),
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
#[error("the quoted tokens are the same")]
pub struct SameTokens;
