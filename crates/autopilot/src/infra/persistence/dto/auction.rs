use {
    super::order::Order,
    crate::domain::{self, auction::Price, eth},
    alloy::primitives::{Address, U256},
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

pub fn from_domain(auction: domain::RawAuctionData) -> RawAuctionData {
    RawAuctionData {
        block: auction.block,
        orders: auction
            .orders
            .into_iter()
            .map(super::order::from_domain)
            .collect(),
        prices: auction
            .prices
            .into_iter()
            .map(|(key, value)| (key.0, value.get().0))
            .collect(),
        surplus_capturing_jit_order_owners: auction
            .surplus_capturing_jit_order_owners
            .into_iter()
            .collect(),
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAuctionData {
    pub block: u64,
    pub orders: Vec<Order>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<Address, U256>,
    #[serde(default)]
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

pub type AuctionId = i64;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub id: AuctionId,
    #[serde(flatten)]
    pub auction: RawAuctionData,
}

impl Auction {
    pub fn try_into_domain(self) -> anyhow::Result<domain::Auction> {
        Ok(domain::Auction {
            id: self.id,
            block: self.auction.block,
            orders: self
                .auction
                .orders
                .into_iter()
                .map(super::order::to_domain)
                .collect(),
            prices: self
                .auction
                .prices
                .into_iter()
                .map(|(key, value)| {
                    Price::try_new(value.into()).map(|price| (eth::TokenAddress(key), price))
                })
                .collect::<Result<_, _>>()?,
            surplus_capturing_jit_order_owners: self
                .auction
                .surplus_capturing_jit_order_owners
                .into_iter()
                .collect(),
        })
    }
}
