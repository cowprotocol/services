use {
    crate::{domain, infra::database::quotes::postgres::dto::InvalidConversion},
    model::order::OrderUid,
    std::collections::HashMap,
};

pub fn to_domain(
    auction: model::auction::Auction,
    quotes: HashMap<OrderUid, Result<domain::Quote, InvalidConversion>>,
    fee_policy: &domain::fee::Policies,
) -> domain::auction::Auction {
    domain::auction::Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction
            .orders
            .into_iter()
            .map(|order| {
                let quote = match quotes.get(&order.metadata.uid) {
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
                super::order::to_domain(order, quote, Some(fee_policy))
            })
            .collect(),
        prices: auction.prices,
    }
}
