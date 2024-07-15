use {
    crate::{auction::AuctionId, Address},
    sqlx::PgConnection,
};

pub async fn insert(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    surplus_capturing_jit_order_owners: &[Address],
) -> Result<(), sqlx::Error> {
    const QUERY: &str =
        r#"INSERT INTO surplus_capturing_jit_order_owners (auction_id, owners) VALUES ($1, $2);"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(surplus_capturing_jit_order_owners)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Option<Vec<Address>>, sqlx::Error> {
    const QUERY: &str =
        r#"SELECT owners FROM surplus_capturing_jit_order_owners WHERE auction_id = $1;"#;
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

        let auction = vec![ByteArray([1; 20]), ByteArray([2; 20])];

        insert(&mut db, 1, &auction).await.unwrap();

        let output = fetch(&mut db, 1).await.unwrap();
        assert_eq!(output, Some(auction));

        // non-existent auction
        let output = fetch(&mut db, 2).await.unwrap();
        assert!(output.is_none());
    }
}
