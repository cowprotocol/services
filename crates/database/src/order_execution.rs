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
    executed_fee: &BigDecimal,
) -> Result<(), sqlx::Error> {
    let surplus_fee: Option<&BigDecimal> = None;
    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, surplus_fee, solver_fee)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (order_uid, auction_id) DO UPDATE
SET reward = $3, surplus_fee = $4, solver_fee = $5
    ;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(surplus_fee)
        .bind(Some(executed_fee))
        .execute(ex)
        .await?;
    Ok(())
}

/// Updates the executed fee for the filled order in the auction.
/// Populates the `solver_fee` column in the `order_execution` table. // TODO:
/// rename to "executed_fee"
pub async fn upsert_executed_fee(
    mut ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    executed_fee: &BigDecimal,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE order_execution
SET solver_fee = $1
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

        save(&mut db, &Default::default(), 0, &BigDecimal::from(5))
            .await
            .unwrap();
    }
}
