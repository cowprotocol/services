use {
    super::{eth, Order},
    std::collections::HashMap,
};

pub mod order;

/// Replicates [`crate::model::Auction`].
#[derive(Clone, Debug, PartialEq)]
pub struct AuctionWithoutId {
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    pub prices: Prices,
    pub surplus_capturing_jit_order_owners: Vec<eth::Address>,
}

pub type Id = i64;

#[derive(Clone, Debug)]
pub struct Auction {
    pub id: Id,
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    pub prices: Prices,
    pub surplus_capturing_jit_order_owners: Vec<eth::Address>,
}

impl PartialEq for Auction {
    fn eq(&self, other: &Self) -> bool {
        self.block == other.block
            && self.latest_settlement_block == other.latest_settlement_block
            && self.orders == other.orders
            && self.prices == other.prices
            && self.surplus_capturing_jit_order_owners == other.surplus_capturing_jit_order_owners
    }
}

/// The price of a token in wei. This represents how much wei is needed to buy
/// 10**18 of another token.
#[derive(Debug, Clone, Copy, PartialEq)]
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

    // TODO: Remove this method and use the `in_eth` function instead.
    pub fn get(&self) -> eth::Ether {
        self.0
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
    /// let price = Price::new(eth::Ether::from(eth::U256::exp10(15))).unwrap(); // 0.001 ETH
    ///
    /// let eth = price.in_eth(amount);
    /// assert_eq!(eth, eth::Ether::from(eth::U256::exp10(15)));
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
