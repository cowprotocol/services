use {
    crate::{
        domain::{
            competition::{self},
            eth::{self, GasPrice},
            liquidity,
            time,
        },
        infra::{Ethereum, blockchain, solver::Timeouts},
    },
    alloy::primitives::U256,
    std::collections::{HashMap, HashSet},
    thiserror::Error,
};

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
    pub(crate) deadline: chrono::DateTime<chrono::Utc>,
    pub(crate) surplus_capturing_jit_order_owners: HashSet<eth::Address>,
}

impl Auction {
    pub async fn new(
        id: Option<Id>,
        mut orders: Vec<competition::Order>,
        tokens: impl Iterator<Item = Token>,
        deadline: chrono::DateTime<chrono::Utc>,
        eth: &Ethereum,
        surplus_capturing_jit_order_owners: HashSet<eth::Address>,
    ) -> Result<Self, Error> {
        let tokens = Tokens(tokens.map(|token| (token.address, token)).collect());
        let weth = eth.contracts().weth_address();

        // Filter out orders with 0 amounts (can lead to numerical issues)
        // or where the auction doesn't contain information about the traded tokens.
        orders.retain(|order| {
            if order.available().is_zero() {
                tracing::debug!(?order, "filtered out order with 0 amounts");
                return false;
            }
            let all_tokens_present = tokens.0.contains_key(&order.buy.token.as_erc20(weth))
                && tokens.0.contains_key(&order.sell.token);
            if !all_tokens_present {
                tracing::debug!(?order, "filtered out order without token info");
                return false;
            }
            true
        });

        let gas_est = eth.gas_price().await?;
        let gas_price = GasPrice::new(
            U256::from(gas_est.max_fee_per_gas).into(),
            U256::from(gas_est.max_priority_fee_per_gas).into(),
            None,
        );

        Ok(Self {
            id,
            orders,
            tokens,
            gas_price,
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
    pub fn deadline(&self, timeouts: Timeouts) -> time::Deadline {
        time::Deadline::new(self.deadline, timeouts)
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
                eth::U256::from(10).pow(eth::U256::from(18)).into(),
            )))
            .collect()
    }

    pub fn surplus_capturing_jit_order_owners(&self) -> &HashSet<eth::Address> {
        &self.surplus_capturing_jit_order_owners
    }
}

/// The tokens that are used in an auction.
#[derive(Debug, Default, Clone)]
pub struct Tokens(HashMap<eth::TokenAddress, Token>);

impl Tokens {
    pub fn get(&self, address: &eth::TokenAddress) -> Option<&Token> {
        self.0.get(address)
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
    /// let amount = eth::TokenAmount::from(eth::U256::from(10).pow(eth::U256::from(18)));
    /// let price = Price::try_new(eth::Ether::from(
    ///     eth::U256::from(10).pow(eth::U256::from(15)),
    /// ))
    /// .unwrap(); // 0.001 ETH
    ///
    /// let eth = price.in_eth(amount);
    /// assert_eq!(
    ///     eth,
    ///     eth::Ether::from(eth::U256::from(10).pow(eth::U256::from(15)))
    /// );
    /// ```
    pub fn in_eth(self, amount: eth::TokenAmount) -> eth::Ether {
        (amount.0 * self.0.0 / eth::U256::from(Self::BASE)).into()
    }

    /// Convert an amount of ETH into a token amount using this price.
    ///
    /// Converting 1 ETH into a token worth 0.1 ETH (like GNO)
    ///
    /// # Examples
    /// ```
    /// use driver::domain::{competition::auction::Price, eth};
    ///
    /// let amount = eth::Ether::from(eth::U256::from(10).pow(eth::U256::from(18)));
    /// let price = Price::try_new(eth::Ether::from(
    ///     eth::U256::from(10).pow(eth::U256::from(17)),
    /// ))
    /// .unwrap(); // 0.1ETH
    /// assert_eq!(
    ///     price.from_eth(amount),
    ///     eth::U256::from(10).pow(eth::U256::from(19)).into()
    /// );
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
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
}
