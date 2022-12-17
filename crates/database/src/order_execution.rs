use crate::{auction::AuctionId, OrderUid};
use bigdecimal::BigDecimal;
use sqlx::PgConnection;

pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    reward: f64,
    surplus_fee: Option<&BigDecimal>,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, surplus_fee)
VALUES ($1, $2, $3, $4)
    ;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(reward)
        .bind(surplus_fee)
        .execute(ex)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Connection;

    #[tokio::test]
    #[ignore]
    async fn postgres_save() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        save(&mut db, &Default::default(), 0, 0., None)
            .await
            .unwrap();

        save(
            &mut db,
            &Default::default(),
            1,
            0.,
            Some(&Default::default()),
        )
        .await
        .unwrap();
    }
}
