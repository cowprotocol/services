use {
    super::order,
    crate::{
        domain::{
            competition::{self, solution},
            eth,
        },
        infra::{blockchain, Ethereum},
    },
    futures::future::join_all,
    itertools::Itertools,
    std::collections::{HashMap, HashSet},
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

    // Prioritize the orders such that those which are more likely to be fulfilled
    // come before less likely orders. Filter out orders which the trader doesn't
    // have enough balance to pay for.
    //
    // Prioritization is skipped during quoting. It's only used during competition.
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
                // TODO No need to expose Tokens anymore, just use the auction
                // If the orders are of the same kind, then sort by likelihood of fulfillment
                // based on token prices.
                order.likelihood(&self.tokens),
            ))
        });

        // Fetch balances of each token for each trader.
        let tokens_by_trader = self
            .orders
            .iter()
            .flat_map(|order| {
                [
                    (order.trader(), order.sell.token),
                    (order.trader(), order.buy.token),
                ]
            })
            .unique()
            .collect::<HashSet<_>>();
        let mut balances: HashMap<(order::Trader, eth::TokenAddress), eth::TokenAmount> = join_all(
            tokens_by_trader
                .into_iter()
                .map(|(trader, token)| async move {
                    let balance = match eth.balance_of(trader.into(), token).await {
                        Ok(balance) => balance,
                        Err(err) => {
                            tracing::warn!(?trader, ?token, ?err, "failed to fetch balance");
                            return None;
                        }
                    };
                    Some(((trader, token), balance))
                }),
        )
        .await
        .into_iter()
        .flatten()
        .collect();

        // Filter out orders which the trader doesn't have enough balance to pay for.
        self.orders.retain(|order| {
            // Partial orders don't require the trader to hold the full balance, since they
            // don't have to be fully fulfilled.
            if order.is_partial() {
                return true;
            }

            // Sell amounts are withdrawn from the trader's balance when the order settles.
            // In case the trader doesn't have enough balance to pay for the order, filter
            // it out.
            let sell_balance = balances
                .get_mut(&(order.trader(), order.sell.token))
                .unwrap();
            match sell_balance.0.checked_sub(order.sell.amount.into()) {
                Some(remaining) => sell_balance.0 = remaining,
                None => return false,
            }

            // Buy amounts are deposited into the trader's balance when the order settles.
            let buy_balance = balances
                .get_mut(&(order.trader(), order.buy.token))
                .unwrap();
            buy_balance.0 = buy_balance.0.saturating_add(order.buy.amount.into());

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
    // TODO Set this field correctly, currently it isn't being passed into the driver.
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
pub struct Id(i64);

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
