use {
    crate::infra::persistence::dto,
    sqlx::{PgConnection, QueryBuilder},
};

pub async fn insert_batch(
    ex: &mut PgConnection,
    fee_policies: impl IntoIterator<Item = dto::FeePolicy>,
) -> Result<(), sqlx::Error> {
    let mut query_builder = QueryBuilder::new(
        "INSERT INTO fee_policies (auction_id, order_uid, kind, surplus_factor, \
         max_volume_factor, volume_factor) ",
    );

    query_builder.push_values(fee_policies, |mut b, fee_policy| {
        b.push_bind(fee_policy.auction_id)
            .push_bind(fee_policy.order_uid)
            .push_bind(fee_policy.kind)
            .push_bind(fee_policy.surplus_factor)
            .push_bind(fee_policy.max_volume_factor)
            .push_bind(fee_policy.volume_factor);
    });

    query_builder.build().execute(ex).await.map(|_| ())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: dto::AuctionId,
    order_uid: database::OrderUid,
) -> Result<Vec<dto::FeePolicy>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT * FROM fee_policies
        WHERE auction_id = $1 AND order_uid = $2
        ORDER BY application_order
    "#;
    let rows = sqlx::query_as::<_, dto::FeePolicy>(QUERY)
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
    use {super::*, database::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        database::clear_DANGER_(&mut db).await.unwrap();

        // same primary key for all fee policies
        let (auction_id, order_uid) = (1, ByteArray([1; 56]));

        // surplus fee policy without caps
        let fee_policy_1 = dto::FeePolicy {
            auction_id,
            order_uid,
            kind: dto::fee_policy::FeePolicyKind::Surplus,
            surplus_factor: Some(0.1),
            max_volume_factor: Some(1.0),
            volume_factor: None,
        };
        // surplus fee policy with caps
        let fee_policy_2 = dto::FeePolicy {
            auction_id,
            order_uid,
            kind: dto::fee_policy::FeePolicyKind::Surplus,
            surplus_factor: Some(0.2),
            max_volume_factor: Some(0.05),
            volume_factor: None,
        };
        // volume based fee policy
        let fee_policy_3 = dto::FeePolicy {
            auction_id,
            order_uid,
            kind: dto::fee_policy::FeePolicyKind::Volume,
            surplus_factor: None,
            max_volume_factor: None,
            volume_factor: Some(0.06),
        };
        insert_batch(
            &mut db,
            vec![
                fee_policy_1.clone(),
                fee_policy_2.clone(),
                fee_policy_3.clone(),
            ],
        )
        .await
        .unwrap();

        let output = fetch(&mut db, 1, order_uid).await.unwrap();
        assert_eq!(output, vec![fee_policy_1, fee_policy_2, fee_policy_3]);
    }
}
