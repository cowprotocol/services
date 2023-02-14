use {
    crate::{auction::AuctionId, Address},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
};

/// External prices for a given auction.
#[derive(Debug, PartialEq, sqlx::FromRow)]
pub struct Prices {
    pub auction_id: AuctionId,
    pub token: Address,
    pub price: BigDecimal,
}

pub async fn insert(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    token: Address,
    price: BigDecimal,
) -> Result<(), sqlx::Error> {
    const QUERY: &str =
        r#"INSERT INTO auction_prices (auction_id, token, price) VALUES ($1, $2, $3);"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(token)
        .bind(price)
        .execute(ex)
        .await?;
    Ok(())
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

        insert(&mut db, 1, ByteArray([2; 20]), 3.into())
            .await
            .unwrap();
    }
}
