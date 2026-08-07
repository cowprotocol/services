use {
    super::order::{Order, OrdersJson},
    crate::domain::{self, auction::Price},
    alloy::primitives::{Address, U256},
    eth_domain_types as eth,
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

/// Converts the auction into the shape that gets archived to the DB and S3.
///
/// The order list comes in already serialized because those bytes are shared
/// with the `/solve` request (see [`OrdersJson`]).
///
/// Takes the auction by reference so the caller can keep using it afterwards
/// without a deep clone.
pub fn from_domain(
    auction: &domain::RawAuctionData,
    orders: OrdersJson,
) -> RawAuctionData<OrdersJson> {
    RawAuctionData {
        block: auction.block,
        orders,
        prices: auction
            .prices
            .iter()
            .map(|(key, value)| (**key, value.get().0))
            .collect(),
        surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners.clone(),
    }
}

/// The archived auction. Generic over the order list so the write path can
/// splice in the pre-rendered [`OrdersJson`] while the read path deserializes
/// into [`Order`]s. One definition, so the two shapes can't drift apart.
///
/// `Deserialize` is only ever instantiated at `O = Vec<Order>`; the derive puts
/// the bound on the impl, not on the struct, so `RawAuctionData<OrdersJson>`
/// simply has no `Deserialize` impl.
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAuctionData<O = Vec<Order>> {
    pub block: u64,
    pub orders: O,
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
                    Price::try_new(value.into()).map(|price| (eth::TokenAddress::from(key), price))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The archived auction is written with pre-rendered orders and read back
    /// into `Vec<Order>`, so the two shapes must stay in sync.
    #[tokio::test]
    async fn archived_auction_round_trips() {
        let token = Address::from([1u8; 20]);
        let auction = domain::RawAuctionData {
            block: 42,
            orders: vec![],
            prices: [(
                token.into(),
                Price::try_new(U256::from(1000).into()).unwrap(),
            )]
            .into_iter()
            .collect(),
            surplus_capturing_jit_order_owners: vec![Address::from([2u8; 20])],
        };

        let written = from_domain(&auction, OrdersJson::new(&auction.orders).await);
        let json = serde_json::to_string(&written).unwrap();

        let read: RawAuctionData = serde_json::from_str(&json).unwrap();
        assert_eq!(read.block, 42);
        assert!(read.orders.is_empty());
        assert_eq!(read.prices, written.prices);
        assert_eq!(
            read.surplus_capturing_jit_order_owners,
            written.surplus_capturing_jit_order_owners
        );
    }
}
