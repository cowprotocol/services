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
        util,
    },
    chrono::Utc,
    solution::trade::ClearingPrices,
    std::collections::{HashMap, HashSet},
};

/// A quote describing the expected outcome of an order.
#[derive(Debug)]
pub struct Quote {
    pub clearing_prices: HashMap<eth::Address, eth::U256>,
    pub pre_interactions: Vec<eth::Interaction>,
    pub interactions: Vec<eth::Interaction>,
    pub solver: eth::Address,
    pub gas: Option<eth::Gas>,
    /// Which `tx.origin` is required to make the quote simulation pass.
    pub tx_origin: Option<eth::Address>,
    pub jit_orders: Vec<solution::trade::Jit>,
}

impl Quote {
    fn try_new(eth: &Ethereum, solution: competition::Solution) -> Result<Self, Error> {
        let clearing_prices = Self::compute_clearing_prices(&solution)?;

        Ok(Self {
            clearing_prices,
            pre_interactions: solution.pre_interactions().to_vec(),
            interactions: solution
                .interactions()
                .iter()
                .map(|i| encode::interaction(i, eth.contracts().settlement().address()))
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

    /// Compute clearing prices for the quote.
    ///
    /// For quotes with trades (fulfillments), use custom clearing prices which
    /// include fees and haircut adjustments. This ensures quotes are
    /// conservative when haircut is enabled.
    ///
    /// For quotes without fulfillments, fall back to uniform clearing prices.
    fn compute_clearing_prices(
        solution: &competition::Solution,
    ) -> Result<HashMap<eth::Address, eth::U256>, Error> {
        // Quote competitions contain only a single order (see `fake_auction()`),
        // so there's at most one fulfillment in the solution.
        let fulfillment = solution.trades().iter().find_map(|trade| match trade {
            solution::Trade::Fulfillment(f) => Some(f),
            solution::Trade::Jit(_) => None,
        });

        match fulfillment {
            Some(fulfillment) => {
                let sell_token = fulfillment.order().sell.token;
                let buy_token = fulfillment.order().buy.token;

                let uniform_prices = ClearingPrices {
                    sell: solution
                        .clearing_price(sell_token)
                        .ok_or(QuotingFailed::ClearingSellMissing)?,
                    buy: solution
                        .clearing_price(buy_token)
                        .ok_or(QuotingFailed::ClearingBuyMissing)?,
                };

                // custom_prices includes haircut via sell_amount adjustment
                let custom_prices = fulfillment
                    .custom_prices(&uniform_prices)
                    .map_err(|_| QuotingFailed::ClearingSellMissing)?;

                // custom_prices.sell = buy_amount (what user receives)
                // custom_prices.buy = sell_amount (what user pays, including haircut)
                // Map to clearing prices: sell_token price = buy_amount, buy_token price =
                // sell_amount
                Ok(HashMap::from([
                    (sell_token.into(), custom_prices.sell),
                    (buy_token.into(), custom_prices.buy),
                ]))
            }
            None => {
                // No fulfillment, use uniform clearing prices
                Ok(solution
                    .clearing_prices()
                    .into_iter()
                    .map(|(token, amount)| (token.into(), amount))
                    .collect())
            }
        }
    }
}

/// An order which needs to be quoted.
#[derive(Debug)]
pub struct Order {
    pub tokens: Tokens,
    pub amount: order::TargetAmount,
    pub side: order::Side,
    pub deadline: chrono::DateTime<chrono::Utc>,
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
                    .fetch(&self.token_liquidity(), infra::liquidity::AtBlock::Recent)
                    .await
            }
            solver::Liquidity::Skip => Default::default(),
        };

        let auction = self
            .fake_auction(eth, tokens, solver.quote_using_limit_orders())
            .await?;
        let solutions = solver.solve(&auction, &liquidity).await?;
        Quote::try_new(
            eth,
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
            auction::Error::Blockchain(e) => e.into(),
        })
    }

    /// The asset being bought, or [`eth::U256::one`] if this is a sell, to
    /// facilitate surplus.
    fn buy(&self) -> eth::Asset {
        match self.side {
            order::Side::Sell => eth::Asset {
                amount: eth::U256::ONE.into(),
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
                // NOTE: the saturating strategy here is slightly irrelevant since we know that 1 <<
                // 144 fits within U256
                amount: (eth::U256::ONE.saturating_shl(144)).into(),
                token: self.tokens.sell,
            },
        }
    }

    /// Returns the token pairs to fetch liquidity for.
    fn token_liquidity(&self) -> HashSet<liquidity::TokenPair> {
        liquidity::TokenPair::try_new(self.tokens.sell(), self.tokens.buy())
            .ok()
            .into_iter()
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Tokens {
    sell: eth::TokenAddress,
    buy: eth::TokenAddress,
}

impl Tokens {
    pub fn new(sell: eth::TokenAddress, buy: eth::TokenAddress) -> Self {
        Self { sell, buy }
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
        alloy::primitives::Address,
        num::rational::Ratio,
    };

    const DEFAULT_QUOTE_SLIPPAGE_BPS: u32 = 100;

    pub(super) fn interaction(
        interaction: &solution::Interaction,
        settlement: &Address,
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
                target: interaction.target.0,
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
