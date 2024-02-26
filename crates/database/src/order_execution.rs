use {
    crate::{auction::AuctionId, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
};

pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    block_number: i64,
    executed_fee: &BigDecimal,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, surplus_fee, block_number)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (order_uid, auction_id)
DO UPDATE SET reward = $3, surplus_fee = $4, block_number = $5
;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(Some(executed_fee))
        .bind(block_number)
        .execute(ex)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_save() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        save(&mut db, &Default::default(), 1, 0, &Default::default())
            .await
            .unwrap();
    }
}
