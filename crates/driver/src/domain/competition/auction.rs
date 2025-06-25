use {
    super::{Order, order},
    crate::{
        domain::{
            competition::{self, sorting},
            eth,
            liquidity,
            time,
        },
        infra::{
            self,
            Ethereum,
            blockchain,
            config::file::OrderPriorityStrategy,
            observe::{self, metrics},
        },
        util::{self},
    },
    chrono::Duration,
    prometheus::HistogramTimer,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    thiserror::Error,
};
use crate::domain::competition::pre_processing;

/// An auction is a set of orders that can be solved. The solvers calculate
/// [`super::solution::Solution`]s by picking subsets of these orders and
/// solving them.
#[derive(Debug, Clone)]
pub struct Auction {
    /// See the [`Self::id`] method.
    pub(crate) id: Option<Id>,
    /// See the [`Self::orders`] method.
    pub(crate) orders: Vec<competition::Order>,
    /// The tokens that are used in the orders of this auction.
    pub(crate) tokens: Tokens,
    pub(crate) gas_price: eth::GasPrice,
    pub(crate) deadline: time::Deadline,
    pub(crate) surplus_capturing_jit_order_owners: HashSet<eth::Address>,
}

impl Auction {
    pub async fn new(
        id: Option<Id>,
        orders: Vec<competition::Order>,
        tokens: impl Iterator<Item = Token>,
        deadline: time::Deadline,
        eth: &Ethereum,
        surplus_capturing_jit_order_owners: HashSet<eth::Address>,
    ) -> Result<Self, Error> {
        let tokens = Tokens(tokens.map(|token| (token.address, token)).collect());

        // Ensure that tokens are included for each order.
        let weth = eth.contracts().weth_address();
        if !orders.iter().all(|order| {
            tokens.0.contains_key(&order.buy.token.as_erc20(weth))
                && tokens.0.contains_key(&order.sell.token)
        }) {
            return Err(Error::InvalidTokens);
        }

        // Ensure that there are no orders with 0 amounts.
        if orders.iter().any(|order| order.available().is_zero()) {
            return Err(Error::InvalidAmounts);
        }

        Ok(Self {
            id,
            orders,
            tokens,
            gas_price: eth.gas_price(None).await?,
            deadline,
            surplus_capturing_jit_order_owners,
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

    /// The tokens used in the auction.
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    /// Returns a collection of liquidity token pairs that are relevant to this
    /// auction.
    pub fn liquidity_pairs(&self) -> HashSet<liquidity::TokenPair> {
        self.orders
            .iter()
            .filter_map(|order| {
                liquidity::TokenPair::try_new(order.sell.token, order.buy.token).ok()
            })
            .collect()
    }

    pub fn gas_price(&self) -> eth::GasPrice {
        self.gas_price
    }

    /// The deadline for the driver to start sending solution to autopilot.
    pub fn deadline(&self) -> time::Deadline {
        self.deadline
    }

    /// Prices used to convert token amounts to an equivalent amount of the
    /// native asset (e.g. ETH on ethereum, or xdai on gnosis chain).
    pub fn native_prices(&self) -> Prices {
        self.tokens
            .0
            .iter()
            .filter_map(|(address, token)| token.price.map(|price| (*address, price)))
            .chain(std::iter::once((
                eth::ETH_TOKEN,
                eth::U256::exp10(18).into(),
            )))
            .collect()
    }

    pub fn surplus_capturing_jit_order_owners(&self) -> &HashSet<eth::Address> {
        &self.surplus_capturing_jit_order_owners
    }
}

#[derive(Clone)]
pub struct AuctionProcessor(Arc<DataAggregator>);

struct DataAggregator {
    eth: infra::Ethereum,
    /// Order sorting strategies should be in the same order as the
    /// `order_priority_strategies` from the driver's config.
    order_sorting_strategies: Vec<Arc<dyn sorting::SortingStrategy>>,
    fetcher: Arc<pre_processing::DataAggregator>,
}

struct AggregatedData {
    balances: Arc<Balances>,
    app_data: Arc<HashMap<order::app_data::AppDataHash, app_data::ValidatedAppData>>,
    cow_amm_orders: Arc<Vec<Order>>,
}

type BalanceGroup = (order::Trader, eth::TokenAddress, order::SellTokenBalance);
type Balances = HashMap<BalanceGroup, order::SellAmount>;

impl AuctionProcessor {
    /// Process the auction by prioritizing the orders and filtering out
    /// unfillable orders. Fetches full app data for each order and returns an
    /// auction with updated orders.
    pub async fn prioritize(&self, auction: Arc<Auction>, solver: eth::H160) -> Auction {
        let _timer = stage_timer("total");

        let agg_data = self.0.clone().fetch_aggregated_data(auction.clone()).await;
        let mut auction = Arc::unwrap_or_clone(auction);

        // This step filters out orders if the an owner doesn't have enough balances for
        // all their orders with the same sell token. That means orders already
        // need to be sorted from most relevant to least relevant so that we
        // allocate balances for the most relevants first.
        //
        // Also use spawn_blocking() because a lot of CPU bound computations are
        // happening and we don't want to block the runtime for too long.
        let helpers = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let _timer = stage_timer("sort_and_update");
            let settlement_contract = helpers.eth.contracts().settlement().address();
            sorting::sort_orders(
                &mut auction.orders,
                &auction.tokens,
                &solver,
                &helpers.order_sorting_strategies,
            );
            Self::update_orders(auction, agg_data, &eth::Address(settlement_contract))
        })
        .await
        .expect(
            "Either runtime was shut down before spawning the task or no OS threads are \
             available; no sense in handling those errors",
        )
    }

    /// Removes orders that cannot be filled due to missing funds of the owner
    /// and updates the fetched app data.
    /// It allocates available funds from left to right so the orders should
    /// already be sorted by priority going in.
    fn update_orders(
        mut auction: Auction,
        data: Arc<AggregatedData>,
        settlement_contract: &eth::Address,
    ) -> Auction {
        // Clone balances since we only aggregate data once but each solver needs
        // to use and modify the data individually.
        let mut balances = data.balances.as_ref().clone();
        let cow_amms: HashSet<_> = data.cow_amm_orders.iter().map(|o| o.uid).collect();

        // The auction that we receive from the `autopilot` assumes that there
        // is sufficient balance to completely cover all the orders. **This is
        // not the case** (as the protocol should not chose which limit orders
        // get filled for some given sell token balance). This loop goes through
        // the priority sorted orders and allocates the available user balance
        // to each order, and potentially scaling the order's `available` amount
        // down in case the available user balance is only enough to partially
        // cover the rest of the order.
        auction.orders.retain_mut(|order| {
            if cow_amms.contains(&order.uid) {
                // cow amm orders already get constructed fully initialized
                // so we don't have to handle them here anymore.
                // Without this short circuiting logic they would get filtered
                // out later because we don't bother fetching their balances
                // for performance reasons.
                return true;
            }

            // Update order app data if it was fetched.
            if let Some(fetched_app_data) = data.app_data.get(&order.app_data.hash()) {
                order.app_data = fetched_app_data.clone().into();
                if order.app_data.flashloan().is_some() {
                    // If an order requires a flashloan we assume all the necessary
                    // sell tokens will come from there. But the receiver must be the
                    // settlement contract because that is how the driver expects
                    // the flashloan to be repaid for now.
                    return order.receiver.as_ref() == Some(settlement_contract);
                }
            }

            let remaining_balance = match balances.get_mut(&(
                order.trader(),
                order.sell.token,
                order.sell_token_balance,
            )) {
                Some(balance) => balance,
                None => {
                    let reason = observe::OrderExcludedFromAuctionReason::CouldNotFetchBalance;
                    observe::order_excluded_from_auction(order, reason);
                    return false;
                }
            };

            let max_sell = order::SellAmount(order.available().sell.amount.0);

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
            if order.available().is_zero() {
                observe::order_excluded_from_auction(
                    order,
                    observe::OrderExcludedFromAuctionReason::OrderWithZeroAmountRemaining,
                );
                return false;
            }

            remaining_balance.0 -= allocated_balance.0;

            true
        });

        auction
    }

    pub fn new(
        eth: &infra::Ethereum,
        order_priority_strategies: Vec<OrderPriorityStrategy>,
        fetcher: Arc<pre_processing::DataAggregator>,
    ) -> Self {
        let eth = eth.with_metric_label("auctionPreProcessing".into());
        let mut order_sorting_strategies = vec![];

        for strategy in order_priority_strategies {
            let comparator: Arc<dyn sorting::SortingStrategy> = match strategy {
                OrderPriorityStrategy::ExternalPrice => Arc::new(sorting::ExternalPrice),
                OrderPriorityStrategy::CreationTimestamp { max_order_age } => {
                    Arc::new(sorting::CreationTimestamp {
                        max_order_age: max_order_age.map(|t| Duration::from_std(t).unwrap()),
                    })
                }
                OrderPriorityStrategy::OwnQuotes { max_order_age } => {
                    Arc::new(sorting::OwnQuotes {
                        max_order_age: max_order_age.map(|t| Duration::from_std(t).unwrap()),
                    })
                }
            };
            order_sorting_strategies.push(comparator);
        }

        Self(
            Arc::new(DataAggregator {
                eth,
                order_sorting_strategies,
                fetcher,
            }))
    }
}

impl DataAggregator {
    async fn fetch_aggregated_data(self: Arc<Self>, auction: Arc<Auction>) -> Arc<AggregatedData> {
        let _timer = stage_timer("aggregate data");
        let start = std::time::Instant::now();

        let tasks = self.fetcher.get_tasks_for_auction(auction.clone());

        let (balances, app_data, cow_amm_orders) = tokio::join!(
            tasks.balances,
            tasks.app_data,
            tasks.cow_amm_orders,
        );
        tracing::debug!(auction_id = ?auction.id(), time =? start.elapsed(), "auction preprocessing done");

        Arc::new(AggregatedData {
            balances,
            app_data,
            cow_amm_orders,
        })
    }
}

fn stage_timer(stage: &str) -> HistogramTimer {
    metrics::get()
        .auction_preprocessing
        .with_label_values(&[stage])
        .start_timer()
}

/// The tokens that are used in an auction.
#[derive(Debug, Default, Clone)]
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
pub struct Price(pub eth::Ether);

impl Price {
    /// The base Ether amount for pricing.
    const BASE: u128 = 10_u128.pow(18);

    pub fn try_new(value: eth::Ether) -> Result<Self, InvalidPrice> {
        if value.0.is_zero() {
            Err(InvalidPrice)
        } else {
            Ok(Self(value))
        }
    }

    /// Apply this price to some token amount, converting that token into ETH.
    ///
    /// # Examples
    ///
    /// Converting 1 ETH expressed in `eth::TokenAmount` into `eth::Ether`
    ///
    /// ```
    /// use driver::domain::{competition::auction::Price, eth};
    ///
    /// let amount = eth::TokenAmount::from(eth::U256::exp10(18));
    /// let price = Price::try_new(eth::Ether::from(eth::U256::exp10(15))).unwrap(); // 0.001 ETH
    ///
    /// let eth = price.in_eth(amount);
    /// assert_eq!(eth, eth::Ether::from(eth::U256::exp10(15)));
    /// ```
    pub fn in_eth(self, amount: eth::TokenAmount) -> eth::Ether {
        (amount.0 * self.0.0 / Self::BASE).into()
    }

    /// Convert an amount of ETH into a token amount using this price.
    ///
    /// Converting 1 ETH into a token worth 0.1 ETH (like GNO)
    ///
    /// # Examples
    /// ```
    /// use driver::domain::{competition::auction::Price, eth};
    ///
    /// let amount = eth::Ether::from(eth::U256::exp10(18));
    /// let price = Price::try_new(eth::Ether::from(eth::U256::exp10(17))).unwrap(); // 0.1ETH
    /// assert_eq!(price.from_eth(amount), eth::U256::exp10(19).into());
    /// ```
    pub fn from_eth(self, amount: eth::Ether) -> eth::TokenAmount {
        (amount.0 * eth::U256::from(Self::BASE) / self.0.0).into()
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

/// All auction prices
pub type Prices = HashMap<eth::TokenAddress, Price>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
