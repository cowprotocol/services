use {
    crate::{auction::AuctionId, OrderUid, PgTransaction},
    sqlx::PgConnection,
    std::ops::DerefMut,
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicyRow {
    pub auction_id: AuctionId,
    pub order_uid: OrderUid,
    pub kind: FeePolicyKindRow,
    pub price_improvement_factor: Option<f64>,
    pub max_volume_factor: Option<f64>,
    pub volume_factor: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "PolicyKind", rename_all = "lowercase")]
pub enum FeePolicyKindRow {
    PriceImprovement,
    Volume,
}

pub async fn insert(
    ex: &mut PgTransaction<'_>,
    fee_policy: FeePolicyRow,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO fee_policies (auction_id, order_uid, kind, price_improvement_factor, max_volume_factor, volume_factor)
        VALUES ($1, $2, $3, $4, $5, $6)
    "#;
    sqlx::query(QUERY)
        .bind(fee_policy.auction_id)
        .bind(fee_policy.order_uid)
        .bind(fee_policy.kind)
        .bind(fee_policy.price_improvement_factor)
        .bind(fee_policy.max_volume_factor)
        .bind(fee_policy.volume_factor)
        .execute(ex.deref_mut())
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    order_uid: OrderUid,
) -> Result<Vec<FeePolicyRow>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT * FROM fee_policies
        WHERE auction_id = $1 AND order_uid = $2
        ORDER BY application_order
    "#;
    let rows = sqlx::query_as::<_, FeePolicyRow>(QUERY)
        .bind(auction_id)
        .bind(order_uid)
        .fetch_all(ex)
        .await?
        .into_iter()
        .collect();
    Ok(rows)
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

        // same primary key for all fee policies
        let (auction_id, order_uid) = (1, ByteArray([1; 56]));

        // price improvement fee policy without caps
        let fee_policy_1 = FeePolicyRow {
            auction_id,
            order_uid,
            kind: FeePolicyKindRow::PriceImprovement,
            price_improvement_factor: Some(0.1),
            max_volume_factor: Some(1.0),
            volume_factor: None,
        };
        insert(&mut db, fee_policy_1.clone()).await.unwrap();

        // price improvement fee policy with caps
        let fee_policy_2 = FeePolicyRow {
            auction_id,
            order_uid,
            kind: FeePolicyKindRow::PriceImprovement,
            price_improvement_factor: Some(0.2),
            max_volume_factor: Some(0.05),
            volume_factor: None,
        };
        insert(&mut db, fee_policy_2.clone()).await.unwrap();

        // volume based fee policy
        let fee_policy_3 = FeePolicyRow {
            auction_id,
            order_uid,
            kind: FeePolicyKindRow::Volume,
            price_improvement_factor: None,
            max_volume_factor: None,
            volume_factor: Some(0.06),
        };
        insert(&mut db, fee_policy_3.clone()).await.unwrap();

        let output = fetch(&mut db, 1, order_uid).await.unwrap();
        assert_eq!(output, vec![fee_policy_1, fee_policy_2, fee_policy_3]);
    }
}
