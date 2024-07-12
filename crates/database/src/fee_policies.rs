use {
    crate::{auction::AuctionId, OrderUid},
    sqlx::{PgConnection, QueryBuilder},
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Deserialize)]
pub struct FeePolicy {
    pub auction_id: AuctionId,
    pub order_uid: OrderUid,
    pub kind: FeePolicyKind,
    pub surplus_factor: Option<f64>,
    pub surplus_max_volume_factor: Option<f64>,
    pub volume_factor: Option<f64>,
    pub price_improvement_factor: Option<f64>,
    pub price_improvement_max_volume_factor: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, sqlx::Type, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "PolicyKind", rename_all = "lowercase")]
pub enum FeePolicyKind {
    Surplus,
    Volume,
    PriceImprovement,
}

pub async fn insert_batch(
    ex: &mut PgConnection,
    fee_policies: impl IntoIterator<Item = FeePolicy>,
) -> Result<(), sqlx::Error> {
    let mut fee_policies = fee_policies.into_iter().peekable();
    if fee_policies.peek().is_none() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::new(
        "INSERT INTO fee_policies (auction_id, order_uid, kind, surplus_factor, \
         surplus_max_volume_factor, volume_factor, price_improvement_factor, \
         price_improvement_max_volume_factor)",
    );

    query_builder.push_values(fee_policies, |mut b, fee_policy| {
        b.push_bind(fee_policy.auction_id)
            .push_bind(fee_policy.order_uid)
            .push_bind(fee_policy.kind)
            .push_bind(fee_policy.surplus_factor)
            .push_bind(fee_policy.surplus_max_volume_factor)
            .push_bind(fee_policy.volume_factor)
            .push_bind(fee_policy.price_improvement_factor)
            .push_bind(fee_policy.price_improvement_max_volume_factor);
    });

    query_builder.build().execute(ex).await.map(|_| ())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    order_uid: OrderUid,
) -> Result<Vec<FeePolicy>, sqlx::Error> {
    const QUERY: &str = r#"
    SELECT * FROM fee_policies
    WHERE auction_id = $1 AND order_uid = $2
    ORDER BY application_order
"#;
    let rows = sqlx::query_as::<_, FeePolicy>(QUERY)
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

        // surplus fee policy without caps
        let fee_policy_1 = FeePolicy {
            auction_id,
            order_uid,
            kind: FeePolicyKind::Surplus,
            surplus_factor: Some(0.1),
            surplus_max_volume_factor: Some(0.99999),
            volume_factor: None,
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // surplus fee policy with caps
        let fee_policy_2 = FeePolicy {
            auction_id,
            order_uid,
            kind: FeePolicyKind::Surplus,
            surplus_factor: Some(0.2),
            surplus_max_volume_factor: Some(0.05),
            volume_factor: None,
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // volume based fee policy
        let fee_policy_3 = FeePolicy {
            auction_id,
            order_uid,
            kind: FeePolicyKind::Volume,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: Some(0.06),
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // price improvement fee policy
        let fee_policy_4 = FeePolicy {
            auction_id,
            order_uid,
            kind: FeePolicyKind::PriceImprovement,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: None,
            price_improvement_factor: Some(0.1),
            price_improvement_max_volume_factor: Some(0.99999),
        };

        let fee_policies = vec![fee_policy_1, fee_policy_2, fee_policy_3, fee_policy_4];

        insert_batch(&mut db, fee_policies.clone()).await.unwrap();

        let output = fetch(&mut db, 1, order_uid).await.unwrap();
        assert_eq!(output, fee_policies);
    }
}
