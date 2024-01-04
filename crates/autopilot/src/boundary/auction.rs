use crate::domain;

pub fn to_domain(auction: model::auction::Auction) -> domain::Auction {
    domain::Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction
            .orders
            .into_iter()
            .map(super::order::to_domain)
            .collect(),
        prices: auction.prices,
    }
}
