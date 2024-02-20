use {
    super::order::Order,
    crate::domain,
    primitive_types::H160,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

pub fn from_domain(auction: domain::Auction) -> Auction {
    Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction
            .orders
            .into_iter()
            .map(super::order::from_domain)
            .collect(),
        prices: auction
            .prices
            .iter()
            .map(|(key, val)| (*key, (*val).into()))
            .collect(),
    }
}

pub fn to_domain(auction: Auction) -> domain::Auction {
    domain::Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction
            .orders
            .into_iter()
            .map(super::order::to_domain)
            .collect(),
        prices: auction
            .prices
            .iter()
            .map(|(key, val)| (*key, (*val).into()))
            .collect(),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    pub prices: BTreeMap<H160, number::U256>,
}

pub type AuctionId = i64;

impl From<AuctionWithId> for domain::AuctionWithId {
    fn from(dto: AuctionWithId) -> Self {
        domain::AuctionWithId {
            id: dto.id,
            auction: to_domain(dto.auction),
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionWithId {
    pub id: AuctionId,
    #[serde(flatten)]
    pub auction: Auction,
}
