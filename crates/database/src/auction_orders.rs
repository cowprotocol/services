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
