use {
    crate::{auction::AuctionId, Address},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
};

/// External prices for a given auction.
#[derive(Debug, PartialEq, sqlx::FromRow)]
pub struct Prices {
    pub tokens: Vec<Address>,
    pub prices: Vec<BigDecimal>,
}

pub async fn upsert(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    tokens: Vec<Address>,
    prices: Vec<BigDecimal>,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"INSERT INTO auction_prices (auction_id, tokens, prices) VALUES ($1, $2, $3)
        ON CONFLICT (auction_id) DO UPDATE
        SET tokens = $2, prices = $3
        ;"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(tokens)
        .bind(prices)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Option<Prices>, sqlx::Error> {
    const QUERY: &str = r#"SELECT tokens, prices FROM auction_prices WHERE
auction_id = $1;"#;
    let prices = sqlx::query_as(QUERY)
        .bind(auction_id)
        .fetch_optional(ex)
        .await?;
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

        // insert full list of tokens
        upsert(
            &mut db,
            1,
            vec![ByteArray([2; 20]), ByteArray([3; 20])],
            vec![4.into(), 5.into()],
        )
        .await
        .unwrap();
        let prices = fetch(&mut db, 1).await.unwrap().unwrap();
        assert_eq!(
            prices,
            Prices {
                tokens: vec![ByteArray([2; 20]), ByteArray([3; 20])],
                prices: vec![4.into(), 5.into()],
            }
        );

        // update with reduces number of tokens
        upsert(&mut db, 1, vec![ByteArray([2; 20])], vec![4.into()])
            .await
            .unwrap();
        let prices = fetch(&mut db, 1).await.unwrap().unwrap();
        assert_eq!(
            prices,
            Prices {
                tokens: vec![ByteArray([2; 20])],
                prices: vec![4.into()],
            }
        );

        // non-existent auction
        let prices = fetch(&mut db, 2).await.unwrap();
        assert_eq!(prices, None);
    }
}
