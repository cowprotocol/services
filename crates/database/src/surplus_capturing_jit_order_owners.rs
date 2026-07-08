use {
    crate::{Address, auction::AuctionId},
    sqlx::PgConnection,
    tracing::instrument,
};

#[instrument(skip_all)]
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
