use {
    crate::{auction::AuctionId, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
};

pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    executed_fee: &BigDecimal,
) -> Result<(), sqlx::Error> {
    let scoring_fee: Option<&BigDecimal> = None; // we don't need a scoring fee anymore
    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, surplus_fee, solver_fee)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (order_uid, auction_id)
DO UPDATE SET reward = $3, surplus_fee = $4, solver_fee = $5
;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(Some(executed_fee))
        .bind(scoring_fee)
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

        save(&mut db, &Default::default(), 0, &Default::default())
            .await
            .unwrap();
    }
}
