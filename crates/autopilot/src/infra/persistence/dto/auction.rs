use {
    super::order::Order,
    crate::{
        domain,
        domain::{auction::Price, eth},
    },
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

pub fn from_domain(auction: domain::RawAuctionData) -> RawAuctionData {
    RawAuctionData {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction
            .orders
            .into_iter()
            .map(super::order::from_domain)
            .collect(),
        prices: auction
            .prices
            .into_iter()
            .map(|(key, value)| (key.into(), value.get().into()))
            .collect(),
        surplus_capturing_jit_order_owners: auction
            .surplus_capturing_jit_order_owners
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename = "Auction")]
#[serde(rename_all = "camelCase")]
pub struct RawAuctionData {
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
    #[serde(default)]
    pub surplus_capturing_jit_order_owners: Vec<H160>,
}

pub type AuctionId = i64;

impl TryFrom<Auction> for domain::Auction {
    type Error = anyhow::Error;

    fn try_from(dto: Auction) -> anyhow::Result<Self> {
        Ok(domain::Auction {
            id: dto.id,
            block: dto.auction.block,
            latest_settlement_block: dto.auction.latest_settlement_block,
            orders: dto
                .auction
                .orders
                .into_iter()
                .map(super::order::to_domain)
                .collect(),
            prices: dto
                .auction
                .prices
                .into_iter()
                .map(|(key, value)| {
                    Price::new(value.into()).map(|price| (eth::TokenAddress(key), price))
                })
                .collect::<Result<_, _>>()?,
            surplus_capturing_jit_order_owners: dto
                .auction
                .surplus_capturing_jit_order_owners
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename = "AuctionWithId")]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub id: AuctionId,
    #[serde(flatten)]
    pub auction: RawAuctionData,
}
