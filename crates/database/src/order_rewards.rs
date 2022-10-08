use crate::{auction::AuctionId, OrderUid};
use sqlx::PgConnection;

pub async fn save(
    ex: &mut PgConnection,
    order: OrderUid,
    auction: AuctionId,
    reward: f64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO order_rewards (order_uid, auction_id, reward)
VALUES ($1, $2, $3)
    ;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(reward)
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

        save(
            &mut db,
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .await
        .unwrap();
    }
}
