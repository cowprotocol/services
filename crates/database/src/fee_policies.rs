use {
    crate::{auction::AuctionId, OrderUid},
    sqlx::{PgConnection, QueryBuilder},
    std::collections::HashMap,
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
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

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
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

pub async fn fetch_all(
    ex: &mut PgConnection,
    keys_filter: &[(AuctionId, OrderUid)],
) -> Result<HashMap<(AuctionId, OrderUid), Vec<FeePolicy>>, sqlx::Error> {
    if keys_filter.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query_builder = QueryBuilder::new("SELECT * FROM fee_policies WHERE ");
    for (i, (auction_id, order_uid)) in keys_filter.iter().enumerate() {
        if i > 0 {
            query_builder.push(" OR ");
        }
        query_builder
            .push("(")
            .push("auction_id = ")
            .push_bind(auction_id)
            .push(" AND ")
            .push("order_uid = ")
            .push_bind(order_uid)
            .push(")");
    }

    query_builder.push(" ORDER BY application_order");

    let query = query_builder.build_query_as::<FeePolicy>();
    let rows = query.fetch_all(ex).await?;
    let mut result: HashMap<(AuctionId, OrderUid), Vec<FeePolicy>> = HashMap::new();
    for row in rows {
        let key = (row.auction_id, row.order_uid);
        result.entry(key).or_default().push(row);
    }

    Ok(result)
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
        let (auction_id_a, order_uid_a) = (1, ByteArray([1; 56]));
        let (auction_id_b, order_uid_b) = (2, ByteArray([2; 56]));

        let output = fetch_all(
            &mut db,
            &[(auction_id_a, order_uid_a), (auction_id_b, order_uid_b)],
        )
        .await
        .unwrap();
        assert!(output.is_empty());

        // surplus fee policy without caps
        let fee_policy_1 = FeePolicy {
            auction_id: auction_id_a,
            order_uid: order_uid_a,
            kind: FeePolicyKind::Surplus,
            surplus_factor: Some(0.1),
            surplus_max_volume_factor: Some(0.99999),
            volume_factor: None,
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // surplus fee policy with caps
        let fee_policy_2 = FeePolicy {
            auction_id: auction_id_b,
            order_uid: order_uid_b,
            kind: FeePolicyKind::Surplus,
            surplus_factor: Some(0.2),
            surplus_max_volume_factor: Some(0.05),
            volume_factor: None,
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // volume based fee policy
        let fee_policy_3 = FeePolicy {
            auction_id: auction_id_b,
            order_uid: order_uid_b,
            kind: FeePolicyKind::Volume,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: Some(0.06),
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // price improvement fee policy
        let fee_policy_4 = FeePolicy {
            auction_id: auction_id_a,
            order_uid: order_uid_a,
            kind: FeePolicyKind::PriceImprovement,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: None,
            price_improvement_factor: Some(0.1),
            price_improvement_max_volume_factor: Some(0.99999),
        };

        let fee_policies = vec![
            fee_policy_1.clone(),
            fee_policy_2.clone(),
            fee_policy_3.clone(),
            fee_policy_4.clone(),
        ];
        insert_batch(&mut db, fee_policies.clone()).await.unwrap();

        let mut expected = HashMap::new();
        expected.insert(
            (auction_id_a, order_uid_a),
            vec![fee_policy_1, fee_policy_4],
        );
        let output = fetch_all(&mut db, &[(auction_id_a, order_uid_a)])
            .await
            .unwrap();
        assert_eq!(output, expected);

        expected.insert(
            (auction_id_b, order_uid_b),
            vec![fee_policy_2, fee_policy_3],
        );
        let output = fetch_all(
            &mut db,
            &[(auction_id_a, order_uid_a), (auction_id_b, order_uid_b)],
        )
        .await
        .unwrap();
        assert_eq!(output, expected);
    }
}
