use {
    crate::{auction::AuctionId, Address, PgTransaction},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
    std::ops::DerefMut,
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicy {
    pub auction_id: AuctionId,
    pub order_uid: OrderUid,
    pub quote_deviation_factor: Option<f64>,
    pub volume_factor: Option<f64>,
    pub absolute_fee: Option<BigDecimal>, 
}

pub async fn insert(ex: &mut PgTransaction<'_>, fee_policy: FeePolicy) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO fee_policies (auction_id, order_uid, quote_deviation_factor, volume_factor, absolute_fee)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (auction_id, order_uid)
        DO UPDATE SET quote_deviation_factor = $3, volume_factor = $4, absolute_fee = $5
    "#;
    sqlx::query(QUERY)
        .bind(fee_policy.auction_id)
        .bind(fee_policy.order_uid)
        .bind(fee_policy.quote_deviation_factor)
        .bind(fee_policy.volume_factor)
        .bind(fee_policy.absolute_fee)
        .execute(ex.deref_mut())
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    order_uid: OrderUid,
) -> Result<Option<FeePolicy>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT auction_id, order_uid, quote_deviation_factor, volume_factor, absolute_fee
        FROM fee_policies
        WHERE auction_id = $1 AND order_uid = $2
    "#;
    let fee_policy = sqlx::query_as::<_, FeePolicy>(QUERY)
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

        let input = FeePolicy {
            auction_id: 1,
            order_uid: OrderUid::new(ByteArray::from([1; 16])),
            quote_deviation_factor: Some(0.1),
            volume_factor: Some(0.2),
            absolute_fee: Some(BigDecimal::from(5)),
        };
        insert(&mut db, input.clone()).await.unwrap();

        let output = fetch(&mut db, 1).await.unwrap().unwrap();
        assert_eq!(input, output);
    }
}
