use {
    super::order,
    crate::{
        domain::{
            competition::{self, solution},
            eth::{self},
        },
        infra::{
            blockchain,
            observe::{self},
            Ethereum,
        },
    },
    futures::future::join_all,
    itertools::Itertools,
    primitive_types::U256,
    std::collections::HashMap,
    thiserror::Error,
};

/// An auction is a set of orders that can be solved. The solvers calculate
/// [`super::solution::Solution`]s by picking subsets of these orders and
/// solving them.
#[derive(Debug)]
pub struct Auction {
    /// See the [`Self::id`] method.
    id: Option<Id>,
    /// See the [`Self::orders`] method.
    orders: Vec<competition::Order>,
    /// The tokens that are used in the orders of this auction.
    tokens: Tokens,
    gas_price: eth::GasPrice,
    deadline: Deadline,
}

impl Auction {
    pub async fn new(
        id: Option<Id>,
        orders: Vec<competition::Order>,
        tokens: impl Iterator<Item = Token>,
        deadline: Deadline,
        eth: &Ethereum,
    ) -> Result<Self, Error> {
        let tokens = Tokens(tokens.map(|token| (token.address, token)).collect());

        // Ensure that tokens are included for each order.
        let weth = eth.contracts().weth_address();
        if !orders.iter().all(|order| {
            tokens.0.contains_key(&order.buy.token.wrap(weth))
                && tokens.0.contains_key(&order.sell.token.wrap(weth))
        }) {
            return Err(Error::InvalidTokens);
        }

        Ok(Self {
            id,
            orders,
            tokens,
            gas_price: eth.gas_price().await?,
            deadline,
        })
    }

    /// [`None`] if this auction applies to a quote. See
    /// [`crate::domain::quote`].
    pub fn id(&self) -> Option<Id> {
        self.id
    }

    /// The orders for the auction.
    pub fn orders(&self) -> &[competition::Order] {
        &self.orders
    }

    /// Prioritize the orders such that those which are more likely to be
    /// fulfilled come before less likely orders. Filter out orders which
    /// the trader doesn't have enough balance to pay for.
    ///
    /// Prioritization is skipped during quoting. It's only used during
    /// competition.
    pub async fn prioritize(mut self, eth: &Ethereum) -> Self {
        // Sort orders so that most likely to be fulfilled come first.
        self.orders.sort_by_key(|order| {
            // Market orders are preferred over limit orders, as the expectation is that
            // they should be immediately fulfillable. Liquidity orders come last, as they
            // are the most niche and rarely used.
            let class = match order.kind {
                competition::order::Kind::Market => 2,
                competition::order::Kind::Limit { .. } => 1,
                competition::order::Kind::Liquidity => 0,
            };
            std::cmp::Reverse((
                class,
                // If the orders are of the same kind, then sort by likelihood of fulfillment
                // based on token prices.
                order.likelihood(&self.tokens),
            ))
        });

        // Fetch balances of each token for each trader.
        // Has to be separate closure due to compiler bug.
        let f = |order: &competition::Order| -> (order::Trader, eth::TokenAddress) {
            (order.trader(), order.sell.token)
        };
        let tokens_by_trader = self.orders.iter().map(f).unique();
        let mut balances: HashMap<
            (order::Trader, eth::TokenAddress),
            Result<eth::TokenAmount, crate::infra::blockchain::Error>,
        > = join_all(tokens_by_trader.map(|(trader, token)| async move {
            let contract = eth.erc20(token);
            let balance = contract.balance(trader.into()).await;
            ((trader, token), balance)
        }))
        .await
        .into_iter()
        .collect();

        self.orders.retain(|order| {
            // TODO: We should use balance fetching that takes interactions into account
            // from `crates/shared/src/account_balances/simulation.rs` instead of hardcoding
            // an Ethflow exception. https://github.com/cowprotocol/services/issues/1595
            if Some(order.signature.signer.0) == eth.contracts().ethflow_address().map(|a| a.0) {
                return true;
            }

            let remaining_balance = match balances
                .get_mut(&(order.trader(), order.sell.token))
                .unwrap()
            {
                Ok(balance) => &mut balance.0,
                Err(err) => {
                    let reason = observe::OrderExcludedFromAuctionReason::CouldNotFetchBalance(err);
                    observe::order_excluded_from_auction(order, reason);
                    return false;
                }
            };

            fn max_fill(order: &competition::Order) -> anyhow::Result<U256> {
                use {
                    anyhow::Context,
                    shared::remaining_amounts::{Order as RemainingOrder, Remaining},
                };

                let remaining = Remaining::from_order(&RemainingOrder {
                    kind: match order.side {
                        order::Side::Buy => model::order::OrderKind::Buy,
                        order::Side::Sell => model::order::OrderKind::Sell,
                    },
                    buy_amount: order.buy.amount.0,
                    sell_amount: order.sell.amount.0,
                    fee_amount: order.fee.user.0,
                    executed_amount: match order.partial {
                        order::Partial::Yes { executed } => executed.0,
                        order::Partial::No => 0.into(),
                    },
                    partially_fillable: match order.partial {
                        order::Partial::Yes { .. } => true,
                        order::Partial::No => false,
                    },
                })
                .context("Remaining::from_order")?;
                let sell = remaining
                    .remaining(order.sell.amount.0)
                    .context("remaining_sell")?;
                let fee = remaining
                    .remaining(order.fee.user.0)
                    .context("remaining_fee")?;
                sell.checked_add(fee).context("add sell and fee")
            }

            let max_fill = match max_fill(order) {
                Ok(balance) => balance,
                Err(err) => {
                    let reason =
                        observe::OrderExcludedFromAuctionReason::CouldNotCalculateRemainingAmount(
                            &err,
                        );
                    observe::order_excluded_from_auction(order, reason);
                    return false;
                }
            };

            let used_balance = match order.is_partial() {
                true => {
                    if *remaining_balance == 0.into() {
                        return false;
                    }
                    max_fill.min(*remaining_balance)
                }
                false => {
                    if *remaining_balance < max_fill {
                        return false;
                    }
                    max_fill
                }
            };
            *remaining_balance -= used_balance;
            true
        });

        self
    }

    /// The tokens used in the auction.
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    pub fn gas_price(&self) -> eth::GasPrice {
        self.gas_price
    }

    pub fn deadline(&self) -> Deadline {
        self.deadline
    }
}

/// The tokens that are used in an auction.
#[derive(Debug, Default)]
pub struct Tokens(HashMap<eth::TokenAddress, Token>);

impl Tokens {
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
    /// The balance of this token available in our settlement contract.
    pub available_balance: eth::U256,
    /// Is this token well-known and trusted by the protocol?
    pub trusted: bool,
}

/// The price of a token in wei. This represents how much wei is needed to buy
/// 10**18 of another token.
#[derive(Debug, Clone, Copy)]
pub struct Price(eth::Ether);

impl Price {
    pub fn new(value: eth::Ether) -> Result<Self, InvalidPrice> {
        if value.0.is_zero() {
            Err(InvalidPrice)
        } else {
            Ok(Self(value))
        }
    }

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
pub struct Id(pub i64);

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

#[derive(Debug, Error)]
#[error("price cannot be zero")]
pub struct InvalidPrice;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid auction tokens")]
    InvalidTokens,
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
}
