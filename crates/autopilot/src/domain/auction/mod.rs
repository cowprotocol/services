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
pub struct Price(pub eth::Ether);

impl Price {
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

/// All auction prices
pub type Prices = HashMap<eth::TokenAddress, Price>;
