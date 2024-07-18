use {
    super::order::Order,
    crate::{
        domain,
        domain::auction::{Price, Prices},
    },
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
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

pub fn try_to_domain(auction: Auction) -> anyhow::Result<domain::Auction> {
    Ok(domain::Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction
            .orders
            .into_iter()
            .map(super::order::to_domain)
            .collect(),
        prices: {
            let mut result = Prices::new();
            auction.prices.into_iter().try_for_each(|(key, value)| {
                result.insert(key.into(), Price::new(value.into())?);
                Ok::<_, anyhow::Error>(())
            })?;
            result
        },
        surplus_capturing_jit_order_owners: auction
            .surplus_capturing_jit_order_owners
            .into_iter()
            .map(Into::into)
            .collect(),
    })
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
    #[serde(default)]
    pub surplus_capturing_jit_order_owners: Vec<H160>,
}

pub type AuctionId = i64;

impl TryFrom<AuctionWithId> for domain::AuctionWithId {
    type Error = anyhow::Error;

    fn try_from(dto: AuctionWithId) -> anyhow::Result<Self> {
        Ok(domain::AuctionWithId {
            id: dto.id,
            auction: try_to_domain(dto.auction)?,
        })
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
