use {
    super::order,
    crate::{
        domain::{
            competition::{self, solution},
            eth,
        },
        infra::{blockchain, observe, Ethereum},
        util,
    },
    futures::future::join_all,
    itertools::Itertools,
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
                && tokens.0.contains_key(&order.sell.token)
        }) {
            return Err(Error::InvalidTokens);
        }

        // Ensure that there are no orders with 0 amounts.
        if orders.iter().any(|order| order.available(weth).is_zero()) {
            return Err(Error::InvalidAmounts);
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

        // Collect trader/token/source/interaction tuples for fetching available
        // balances. Note that we are pessimistic here, if a trader is selling
        // the same token with the same source in two different orders using a
        // different set of pre-interactions, then we fetch the balance as if no
        // pre-interactions were specified. This is done to avoid creating
        // dependencies between orders (i.e. order 1 is required for executing
        // order 2) which we currently cannot express with the solver interface.
        let traders = self
            .orders()
            .iter()
            .group_by(|order| (order.trader(), order.sell.token, order.sell_token_balance))
            .into_iter()
            .map(|((trader, token, source), mut orders)| {
                let first = orders.next().expect("group contains at least 1 order");
                let mut others = orders;
                if others.all(|order| order.pre_interactions == first.pre_interactions) {
                    (trader, token, source, &first.pre_interactions[..])
                } else {
                    (trader, token, source, Default::default())
                }
            })
            .collect::<Vec<_>>();

        let mut balances = join_all(traders.into_iter().map(
            |(trader, token, source, interactions)| async move {
                let balance = eth
                    .erc20(token)
                    .tradable_balance(trader.into(), source, interactions)
                    .await;
                (
                    (trader, token, source),
                    balance.map(order::SellAmount::from),
                )
            },
        ))
        .await
        .into_iter()
        .collect::<HashMap<_, _>>();

        // The auction that we receive from the `autopilot` assumes that there
        // is sufficient balance to completely cover all the orders. **This is
        // not the case** (as the protocol should not chose which limit orders
        // get filled for some given sell token balance). This loop goes through
        // the priority sorted orders and allocates the available user balance
        // to each order, and potentially scaling the order's `available` amount
        // down in case the available user balance is only enough to partially
        // cover the rest of the order.
        let weth = eth.contracts().weth_address();
        self.orders.retain_mut(|order| {
            let remaining_balance = match balances
                .get_mut(&(order.trader(), order.sell.token, order.sell_token_balance))
                .unwrap()
            {
                Ok(balance) => balance,
                Err(err) => {
                    let reason = observe::OrderExcludedFromAuctionReason::CouldNotFetchBalance(err);
                    observe::order_excluded_from_auction(order, reason);
                    return false;
                }
            };

            let max_sell = match {
                let available = order.available(weth);
                available.sell.amount.0.checked_add(available.fee.user.0)
            } {
                Some(amount) => order::SellAmount(amount),
                None => {
                    observe::order_excluded_from_auction(
                        order,
                        observe::OrderExcludedFromAuctionReason::CouldNotCalculateMaxSell,
                    );
                    return false;
                }
            };

            let allocated_balance = match order.partial {
                order::Partial::Yes { .. } => max_sell.min(*remaining_balance),
                order::Partial::No if max_sell <= *remaining_balance => max_sell,
                _ => order::SellAmount::default(),
            };
            if allocated_balance.0.is_zero() {
                observe::order_excluded_from_auction(
                    order,
                    observe::OrderExcludedFromAuctionReason::InsufficientBalance,
                );
                return false;
            }

            // We need to scale the available amount in the order based on
            // allocated balance. We cannot naively just set the `available`
            // amount to equal the `allocated_balance` because of two reasons:
            // 1. They are in different units. `available` is a `TargetAmount` which means
            //    it would be in buy token for buy orders and not in sell token like the
            //    `allocated_balance`
            // 2. Account for fees. Even in the case of sell orders, `available` is
            //    potentially different to `allocated_balance` because of fee scaling. For
            //    example, imagine a partially fillable order selling 100 tokens with a fee
            //    of 10 for a user with a balance of 50. The `allocated_balance` would be 50
            //    tokens, but the `available` amount needs to be less! We want the
            //    following: `available + (fee * available / sell) <= allocated_balance`
            if let order::Partial::Yes { available } = &mut order.partial {
                *available = order::TargetAmount(
                    util::math::mul_ratio(available.0, allocated_balance.0, max_sell.0)
                        .unwrap_or_default(),
                );
            }
            if order.available(weth).is_zero() {
                observe::order_excluded_from_auction(
                    order,
                    observe::OrderExcludedFromAuctionReason::OrderWithZeroAmountRemaining,
                );
                return false;
            }

            remaining_balance.0 -= allocated_balance.0;
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
    #[error("invalid order amounts")]
    InvalidAmounts,
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
}
