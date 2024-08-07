use {
    super::{order, Order},
    crate::{
        domain::{
            competition::{
                self,
                auction,
                sorting::{self, OrderingKey},
            },
            eth::{self},
            liquidity,
            time,
        },
        infra::{self, blockchain, config::file::OrderPriorityStrategy, observe, Ethereum},
        util::{self, Bytes},
    },
    chrono::Utc,
    futures::future::{join_all, BoxFuture, FutureExt, Shared},
    itertools::Itertools,
    model::{order::OrderKind, signature::Signature},
    std::{
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex},
    },
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
    deadline: time::Deadline,
    surplus_capturing_jit_order_owners: HashSet<eth::Address>,
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
            tokens.0.contains_key(&order.buy.token.wrap(weth))
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
            gas_price: eth.gas_price().await?,
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
            .filter_map(|order| match order.kind {
                order::Kind::Market | order::Kind::Limit { .. } => {
                    liquidity::TokenPair::new(order.sell.token, order.buy.token).ok()
                }
                order::Kind::Liquidity => None,
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

    pub fn prices(&self) -> Prices {
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
pub struct AuctionProcessor(Arc<Mutex<Inner>>);

struct Inner {
    auction: auction::Id,
    fut: Shared<BoxFuture<'static, Vec<Order>>>,
    eth: infra::Ethereum,
    order_comparators: Vec<Arc<dyn sorting::OrderComparator>>,
}

type BalanceGroup = (order::Trader, eth::TokenAddress, order::SellTokenBalance);
type Balances = HashMap<BalanceGroup, order::SellAmount>;

impl AuctionProcessor {
    /// Prioritize well priced and filter out unfillable orders from the given
    /// auction.
    pub async fn prioritize(&self, auction: Auction, solver: &eth::H160) -> Auction {
        Auction {
            orders: self.prioritize_orders(&auction, solver).await,
            ..auction
        }
    }

    fn prioritize_orders(
        &self,
        auction: &Auction,
        solver: &eth::H160,
    ) -> Shared<BoxFuture<'static, Vec<Order>>> {
        let new_id = auction
            .id()
            .expect("auctions used for quoting do not have to be prioritized");

        let mut lock = self.0.lock().unwrap();
        let current_id = lock.auction;
        if new_id.0 < current_id.0 {
            tracing::error!(?current_id, ?new_id, "received an outdated auction");
        }
        if current_id.0 == new_id.0 {
            tracing::debug!("await running prioritization task");
            return lock.fut.clone();
        }

        let eth = lock.eth.clone();

        let rt = tokio::runtime::Handle::current();
        let tokens: Tokens = auction.tokens().clone();
        let cow_amms = auction.surplus_capturing_jit_order_owners.clone();
        let mut orders = auction.orders.clone();
        let solver = *solver;
        let order_comparators = lock.order_comparators.clone();

        // Use spawn_blocking() because a lot of CPU bound computations are happening
        // and we don't want to block the runtime for too long.
        let fut = tokio::task::spawn_blocking(move || {
            let start = std::time::Instant::now();
            orders.extend(rt.block_on(Self::cow_amm_orders(&eth, &tokens, &cow_amms)));
            sorting::sort_orders(&mut orders, &tokens, &solver, &order_comparators);
            let mut balances =
                rt.block_on(async { Self::fetch_balances(&eth, &orders).await });
            Self::filter_orders(&mut balances, &mut orders);
            tracing::debug!(auction_id = new_id.0, time =? start.elapsed(), "auction preprocessing done");
            orders
        })
        .map(|res| {
            res.expect(
                "Either runtime was shut down before spawning the task or no OS threads are \
                 available; no sense in handling those errors",
            )
        })
        .boxed()
        .shared();

        tracing::debug!("started new prioritization task");
        lock.auction = new_id;
        lock.fut = fut.clone();

        fut
    }

    /// Removes orders that cannot be filled due to missing funds of the owner.
    fn filter_orders(balances: &mut Balances, orders: &mut Vec<order::Order>) {
        // The auction that we receive from the `autopilot` assumes that there
        // is sufficient balance to completely cover all the orders. **This is
        // not the case** (as the protocol should not chose which limit orders
        // get filled for some given sell token balance). This loop goes through
        // the priority sorted orders and allocates the available user balance
        // to each order, and potentially scaling the order's `available` amount
        // down in case the available user balance is only enough to partially
        // cover the rest of the order.
        orders.retain_mut(|order| {
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
    }

    /// Fetches the tradable balance for every order owner.
    async fn fetch_balances(ethereum: &infra::Ethereum, orders: &[order::Order]) -> Balances {
        let ethereum = ethereum.with_metric_label("orderBalances".into());
        let mut tokens: HashMap<_, _> = Default::default();
        // Collect trader/token/source/interaction tuples for fetching available
        // balances. Note that we are pessimistic here, if a trader is selling
        // the same token with the same source in two different orders using a
        // different set of pre-interactions, then we fetch the balance as if no
        // pre-interactions were specified. This is done to avoid creating
        // dependencies between orders (i.e. order 1 is required for executing
        // order 2) which we currently cannot express with the solver interface.
        let traders = orders
            .iter()
            .group_by(|order| (order.trader(), order.sell.token, order.sell_token_balance))
            .into_iter()
            .map(|((trader, token, source), mut orders)| {
                let first = orders.next().expect("group contains at least 1 order");
                let mut others = orders;
                tokens.entry(token).or_insert_with(|| ethereum.erc20(token));
                if others.all(|order| order.pre_interactions == first.pre_interactions) {
                    (trader, token, source, &first.pre_interactions[..])
                } else {
                    (trader, token, source, Default::default())
                }
            })
            .collect::<Vec<_>>();

        join_all(
            traders
                .into_iter()
                .map(|(trader, token, source, interactions)| {
                    let token_contract = tokens.get(&token);
                    let token_contract = token_contract.expect("all tokens were created earlier");
                    let fetch_balance =
                        token_contract.tradable_balance(trader.into(), source, interactions);

                    async move {
                        let balance = fetch_balance.await;
                        (
                            (trader, token, source),
                            balance.map(order::SellAmount::from).ok(),
                        )
                    }
                }),
        )
        .await
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
        .collect()
    }

    async fn cow_amm_orders(
        eth: &Ethereum,
        tokens: &Tokens,
        eligible_for_surplus: &HashSet<eth::Address>,
    ) -> Vec<Order> {
        let cow_amms = eth.contracts().cow_amm_registry().amms().await;
        let results: Vec<_> = futures::future::join_all(
            cow_amms
                .into_iter()
                // Only generate orders for cow amms the auction told us about.
                // Otherwise the solver would expect the order to get surplus but
                // the autopilot would actually not count it.
                .filter(|amm| eligible_for_surplus.contains(&eth::Address(*amm.address())))
                // Only generate orders where the auction provided the required
                // reference prices. Otherwise there will be an error during the
                // surplus calculation which will also result in 0 surplus for
                // this order.
                .filter_map(|amm| {
                    let prices = amm
                        .traded_tokens()
                        .iter()
                        .map(|t| {
                            tokens
                                .get(eth::TokenAddress(eth::ContractAddress(*t)))
                                .price
                                .map(|p| p.0 .0)
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some((amm, prices))
                })
                .map(|(cow_amm, prices)| async move {
                    (*cow_amm.address(), cow_amm.template_order(prices).await)
                }),
        )
        .await;

        // Convert results to domain format.
        let domain_separator =
            model::DomainSeparator(eth.contracts().settlement_domain_separator().0);
        let orders: Vec<_> = results
            .into_iter()
            .filter_map(|(amm, result)| match result {
                Ok(template) => Some(Order {
                    uid: template.order.uid(&domain_separator, &amm).0.into(),
                    receiver: template.order.receiver.map(|addr| addr.into()),
                    created: Some(
                        u32::try_from(Utc::now().timestamp())
                            .unwrap_or(u32::MAX)
                            .into(),
                    ),
                    valid_to: template.order.valid_to.into(),
                    buy: eth::Asset {
                        amount: template.order.buy_amount.into(),
                        token: template.order.buy_token.into(),
                    },
                    sell: eth::Asset {
                        amount: template.order.sell_amount.into(),
                        token: template.order.sell_token.into(),
                    },
                    kind: order::Kind::Limit,
                    side: template.order.kind.into(),
                    app_data: order::AppData(Bytes(template.order.app_data.0)),
                    buy_token_balance: template.order.buy_token_balance.into(),
                    sell_token_balance: template.order.sell_token_balance.into(),
                    partial: match template.order.partially_fillable {
                        true => order::Partial::Yes {
                            available: match template.order.kind {
                                OrderKind::Sell => order::TargetAmount(template.order.sell_amount),
                                OrderKind::Buy => order::TargetAmount(template.order.buy_amount),
                            },
                        },
                        false => order::Partial::No,
                    },
                    pre_interactions: template
                        .pre_interactions
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    post_interactions: template
                        .post_interactions
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    signature: match template.signature {
                        Signature::Eip1271(bytes) => order::Signature {
                            scheme: order::signature::Scheme::Eip1271,
                            data: Bytes(bytes),
                            signer: amm.into(),
                        },
                        _ => {
                            tracing::warn!(
                                signature = ?template.signature,
                                ?amm,
                                "signature for cow amm order has incorrect scheme"
                            );
                            return None;
                        }
                    },
                    protocol_fees: vec![],
                    quote: None,
                }),
                Err(err) => {
                    tracing::warn!(?err, ?amm, "failed to generate template order for cow amm");
                    None
                }
            })
            .collect();

        if !orders.is_empty() {
            tracing::debug!(?orders, "generated cow amm template orders");
        }

        orders
    }

    pub fn new(
        eth: &infra::Ethereum,
        order_priority_strategies: Vec<OrderPriorityStrategy>,
    ) -> Self {
        let eth = eth.with_metric_label("auctionPreProcessing".into());
        let mut order_comparators = Vec::<Arc<dyn sorting::OrderComparator>>::new();

        order_comparators.push(sorting::OrderClass.comparator());

        for strategy in order_priority_strategies {
            let comparator: Arc<dyn sorting::OrderComparator> = match strategy {
                OrderPriorityStrategy::ExternalPrice => sorting::ExternalPrice.comparator(),
                OrderPriorityStrategy::CreationTimestamp => sorting::CreationTimestamp.comparator(),
                OrderPriorityStrategy::OwnQuotes => sorting::OwnQuotes.comparator(),
            };
            order_comparators.push(comparator);
        }
        Self(Arc::new(Mutex::new(Inner {
            auction: Id(0),
            fut: futures::future::pending().boxed().shared(),
            eth,
            order_comparators,
        })))
    }
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
pub struct Price(eth::Ether);

impl Price {
    /// The base Ether amount for pricing.
    const BASE: u128 = 10_u128.pow(18);

    pub fn new(value: eth::Ether) -> Result<Self, InvalidPrice> {
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
    /// let price = Price::new(eth::Ether::from(eth::U256::exp10(15))).unwrap(); // 0.001 ETH
    ///
    /// let eth = price.in_eth(amount);
    /// assert_eq!(eth, eth::Ether::from(eth::U256::exp10(15)));
    /// ```
    pub fn in_eth(self, amount: eth::TokenAmount) -> eth::Ether {
        (amount.0 * self.0 .0 / Self::BASE).into()
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
    /// let price = Price::new(eth::Ether::from(eth::U256::exp10(17))).unwrap(); // 0.1ETH
    /// assert_eq!(price.from_eth(amount), eth::U256::exp10(19).into());
    /// ```
    pub fn from_eth(self, amount: eth::Ether) -> eth::TokenAmount {
        (amount.0 * eth::U256::from(Self::BASE) / self.0 .0).into()
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
