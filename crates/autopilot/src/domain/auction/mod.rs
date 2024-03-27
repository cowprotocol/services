use {
    super::{eth, Order},
    primitive_types::{H160, U256},
    std::collections::{BTreeMap, HashMap},
};

pub mod order;

/// Replicates [`crate::model::Auction`].
#[derive(Clone, Debug, PartialEq)]
pub struct Auction {
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    pub prices: BTreeMap<H160, U256>,
}

pub type Id = i64;

#[derive(Clone, Debug)]
pub struct AuctionWithId {
    pub id: Id,
    pub auction: Auction,
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
    /// use autopilot::domain::{auction::Price, eth};
    ///
    /// let amount = eth::TokenAmount::from(eth::U256::exp10(18));
    /// let price = Price(eth::Ether::from(eth::U256::exp10(18)));
    ///
    /// let eth = price.in_eth(amount);
    /// assert_eq!(eth, eth::Ether::from(eth::U256::exp10(18)));
    /// ```
    pub fn in_eth(self, amount: eth::TokenAmount) -> eth::Ether {
        (amount.0 * self.0 .0 / Self::BASE).into()
    }
}

/// All auction prices
pub type Prices = HashMap<eth::TokenAddress, Price>;

#[derive(Debug, thiserror::Error)]
#[error("price cannot be zero")]
pub struct InvalidPrice;
