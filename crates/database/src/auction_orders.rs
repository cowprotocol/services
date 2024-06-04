use {
    crate::{auction::AuctionId, OrderUid},
    sqlx::PgConnection,
};

pub async fn insert(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    orders: &[OrderUid],
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"INSERT INTO auction_orders (auction_id, order_uids) VALUES ($1, $2);"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(orders)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Option<Vec<OrderUid>>, sqlx::Error> {
    const QUERY: &str = r#"SELECT order_uids FROM auction_orders WHERE auction_id = $1;"#;
    let row = sqlx::query_scalar(QUERY)
        .bind(auction_id)
        .fetch_optional(ex)
        .await?;
    Ok(row)
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

        let auction = vec![ByteArray([1; 56]), ByteArray([2; 56])];

        insert(&mut db, 1, &auction).await.unwrap();

        let output = fetch(&mut db, 1).await.unwrap();
        assert_eq!(output, Some(auction));

        // non-existent auction
        let output = fetch(&mut db, 2).await.unwrap();
        assert!(output.is_none());
    }
}
