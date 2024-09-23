use {
    crate::{auction::AuctionId, Address, PgTransaction},
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
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

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<AuctionPrice>, sqlx::Error> {
    const QUERY: &str = "SELECT * FROM auction_prices WHERE auction_id = $1";
    let prices = sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await?;
    Ok(prices)
}

pub async fn fetch_latest_prices(ex: &mut PgConnection) -> Result<Vec<AuctionPrice>, sqlx::Error> {
    const QUERY: &str = "SELECT * FROM auction_prices WHERE auction_id = (
        SELECT MAX(auction_id)
        FROM auction_prices
    )";
    let prices = sqlx::query_as(QUERY).fetch_all(ex).await?;
    Ok(prices)
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

        let auction_1 = vec![
            AuctionPrice {
                auction_id: 1,
                token: ByteArray([2; 20]),
                price: 4.into(),
            },
            AuctionPrice {
                auction_id: 1,
                token: ByteArray([3; 20]),
                price: 5.into(),
            },
        ];
        let auction_2 = vec![AuctionPrice {
            auction_id: 2,
            token: ByteArray([4; 20]),
            price: 6.into(),
        }];
        let auction_3 = vec![AuctionPrice {
            auction_id: 3,
            token: ByteArray([5; 20]),
            price: 7.into(),
        }];

        insert(&mut db, &auction_1).await.unwrap();
        insert(&mut db, &auction_2).await.unwrap();
        insert(&mut db, &auction_3).await.unwrap();

        // check that all auctions are there
        let output = fetch(&mut db, 1).await.unwrap();
        assert_eq!(output, auction_1);
        let output = fetch(&mut db, 2).await.unwrap();
        assert_eq!(output, auction_2);
        let output = fetch(&mut db, 3).await.unwrap();
        assert_eq!(output, auction_3);
        // non-existent auction
        let output = fetch(&mut db, 4).await.unwrap();
        assert!(output.is_empty());
    }
}
