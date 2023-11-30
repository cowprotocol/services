use {
    crate::{auction::AuctionId, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
    std::ops::DerefMut,
};

pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    executed_fee: Option<&BigDecimal>,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, executed_fee)
VALUES ($1, $2, $3, $4, $5)
    ;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(executed_fee)
        .execute(ex)
        .await?;
    Ok(())
}

// update already existing order_execution record with executed_fee for partial
// limit orders
pub async fn update_executed_fee(
    mut ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    executed_fee: &BigDecimal,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE order_execution
SET executed_fee = $1
WHERE order_uid = $2 AND auction_id = $3
    ;"#;
    sqlx::query(QUERY)
        .bind(executed_fee)
        .bind(order)
        .bind(auction)
        .execute(ex.deref_mut())
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

        save(&mut db, &Default::default(), 0, None).await.unwrap();

        save(&mut db, &Default::default(), 1, Some(&Default::default()))
            .await
            .unwrap();
    }
}
