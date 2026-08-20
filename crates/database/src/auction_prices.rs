//! This table is deprecated, since it contains duplicated data in the
//! `competition_auctions` table. But it can't currently be removed, since the
//! solver team is still using it.
use {
    crate::{Address, PgTransaction, auction::AuctionId},
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
    std::ops::DerefMut,
    tracing::instrument,
};

/// External token price for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AuctionPrice {
    pub auction_id: AuctionId,
    pub token: Address,
    pub price: BigDecimal,
}

#[instrument(skip_all)]
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

#[instrument(skip_all)]
pub async fn fetch_latest_prices(ex: &mut PgConnection) -> Result<Vec<AuctionPrice>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT * FROM auction_prices WHERE auction_id = (
    SELECT MAX(auction_id)
    FROM auction_prices
)
    "#;
    sqlx::query_as(QUERY).fetch_all(ex).await
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let auction_3 = vec![
            AuctionPrice {
                auction_id: 3,
                token: ByteArray([3; 20]),
                price: 4.into(),
            },
            AuctionPrice {
                auction_id: 3,
                token: ByteArray([4; 20]),
                price: 5.into(),
            },
        ];

        insert(&mut db, &auction_3).await.unwrap();

        // latest prices
        let output = fetch_latest_prices(&mut db).await.unwrap();
        assert_eq!(output, auction_3);
    }
}
