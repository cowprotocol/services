use {
    crate::{domain, infra::persistence::dto::quote::InvalidConversion},
    std::collections::HashMap,
};

pub fn to_domain(
    auction: model::auction::Auction,
    quotes: HashMap<domain::OrderUid, Result<domain::Quote, InvalidConversion>>,
    fee_policy: &domain::fee::Policies,
) -> domain::Auction {
    domain::Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction
            .orders
            .into_iter()
            .map(|order| {
                let quote = match quotes.get(&order.metadata.uid.into()) {
                    None => {
                        tracing::debug!(?order.metadata.uid, "missing quote");
                        None
                    }
                    Some(Err(err)) => {
                        tracing::debug!(?order.metadata.uid, ?err, "invalid quote");
                        None
                    }
                    Some(Ok(quote)) => Some(quote),
                };
                super::order::to_domain(order, quote, fee_policy)
            })
            .collect(),
        prices: auction.prices,
    }
}
