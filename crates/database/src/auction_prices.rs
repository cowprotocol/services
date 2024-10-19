use {
    crate::{auction::AuctionId, Address, PgTransaction},
    bigdecimal::BigDecimal,
    sqlx::QueryBuilder,
    std::ops::DerefMut,
};

/// External token price for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AuctionPrice {
    pub auction_id: AuctionId,
    pub token: Address,
    pub price: BigDecimal,
}

pub async fn insert(
    ex: &mut PgTransaction<'_>,
    prices: &[AuctionPrice],
) -> Result<(), sqlx::Error> {
    const BATCH_SIZE: usize = 5000;
    const QUERY: &str = "INSERT INTO auction_prices (auction_id, token, price) ";

    for chunk in prices.chunks(BATCH_SIZE) {
        let mut query_builder = QueryBuilder::new(QUERY);

        query_builder.push_values(chunk, |mut builder, price| {
            builder
                .push_bind(price.auction_id)
                .push_bind(price.token)
                .push_bind(price.price.clone());
        });

        query_builder.build().execute(ex.deref_mut()).await?;
    }

    Ok(())
}
