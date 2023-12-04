use {
    crate::{auction::AuctionId, OrderUid, PgTransaction},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
    std::ops::DerefMut,
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicies {
    pub auction_id: AuctionId,
    pub order_uid: OrderUid,
    pub price_improvement_factor: Vec<f64>,
    pub volume_factor: Vec<f64>,
    pub absolute_fee: Vec<BigDecimal>,
}

pub async fn insert(
    ex: &mut PgTransaction<'_>,
    fee_policies: FeePolicies,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO fee_policies (auction_id, order_uid, price_improvement_factor, volume_factor, absolute_fee)
        VALUES ($1, $2, $3, $4, $5)
    "#;
    sqlx::query(QUERY)
        .bind(fee_policies.auction_id)
        .bind(fee_policies.order_uid)
        .bind(fee_policies.price_improvement_factor)
        .bind(fee_policies.volume_factor)
        .bind(fee_policies.absolute_fee)
        .execute(ex.deref_mut())
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    order_uid: OrderUid,
) -> Result<Option<FeePolicies>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT auction_id, order_uid, price_improvement_factor, volume_factor, absolute_fee
        FROM fee_policies
        WHERE auction_id = $1 AND order_uid = $2
    "#;
    let fee_policy = sqlx::query_as::<_, FeePolicies>(QUERY)
        .bind(auction_id)
        .bind(order_uid)
        .fetch_optional(ex)
        .await?;
    Ok(fee_policy)
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

        let order_uid = ByteArray([1; 56]);
        let input = FeePolicies {
            auction_id: 1,
            order_uid,
            price_improvement_factor: [0.1, 0.2].to_vec(),
            volume_factor: [0.3, 0.4].to_vec(),
            absolute_fee: [BigDecimal::from(5), BigDecimal::from(6)].to_vec(),
        };
        insert(&mut db, input.clone()).await.unwrap();

        let output = fetch(&mut db, 1, order_uid).await.unwrap().unwrap();
        assert_eq!(input, output);
    }
}
