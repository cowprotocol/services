use {
    crate::{domain, infra::persistence::dto},
    sqlx::{PgConnection, QueryBuilder},
};

pub async fn insert_batch(
    ex: &mut PgConnection,
    auction_id: domain::auction::Id,
    fee_policies: impl IntoIterator<Item = (domain::OrderUid, Vec<domain::fee::Policy>)>,
) -> Result<(), sqlx::Error> {
    let mut fee_policies = fee_policies
        .into_iter()
        .flat_map(|(order_uid, policies)| {
            policies
                .into_iter()
                .map(move |policy| dto::FeePolicy::from_domain(auction_id, order_uid, policy))
        })
        .peekable();

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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::fee::FeeFactor,
        database::byte_array::ByteArray,
        sqlx::Connection,
    };

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

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        database::clear_DANGER_(&mut db).await.unwrap();

        // same primary key for all fee policies
        let (auction_id, order_uid) = (1, ByteArray([1; 56]));

        // surplus fee policy without caps
        let fee_policy_1 = domain::fee::Policy::Surplus {
            factor: FeeFactor::try_from(0.1).unwrap(),
            max_volume_factor: FeeFactor::try_from(0.99999).unwrap(),
        };
        // surplus fee policy with caps
        let fee_policy_2 = domain::fee::Policy::Surplus {
            factor: FeeFactor::try_from(0.2).unwrap(),
            max_volume_factor: FeeFactor::try_from(0.05).unwrap(),
        };
        // volume based fee policy
        let fee_policy_3 = domain::fee::Policy::Volume {
            factor: FeeFactor::try_from(0.06).unwrap(),
        };
        // price improvement fee policy
        let fee_policy_4 = domain::fee::Policy::PriceImprovement {
            factor: FeeFactor::try_from(0.1).unwrap(),
            max_volume_factor: FeeFactor::try_from(0.99999).unwrap(),
            quote: domain::fee::Quote {
                sell_amount: 10.into(),
                buy_amount: 20.into(),
                fee: 1.into(),
            },
        };
        let input_policies = vec![fee_policy_1, fee_policy_2, fee_policy_3, fee_policy_4];

        insert_batch(
            &mut db,
            auction_id,
            vec![(domain::OrderUid(order_uid.0), input_policies)],
        )
        .await
        .unwrap();

        // surplus fee policy without caps
        let fee_policy_1 = dto::FeePolicy {
            auction_id,
            order_uid,
            kind: dto::fee_policy::FeePolicyKind::Surplus,
            surplus_factor: Some(0.1),
            surplus_max_volume_factor: Some(0.99999),
            volume_factor: None,
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // surplus fee policy with caps
        let fee_policy_2 = dto::FeePolicy {
            auction_id,
            order_uid,
            kind: dto::fee_policy::FeePolicyKind::Surplus,
            surplus_factor: Some(0.2),
            surplus_max_volume_factor: Some(0.05),
            volume_factor: None,
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // volume based fee policy
        let fee_policy_3 = dto::FeePolicy {
            auction_id,
            order_uid,
            kind: dto::fee_policy::FeePolicyKind::Volume,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: Some(0.06),
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        };
        // price improvement fee policy
        let fee_policy_4 = dto::FeePolicy {
            auction_id,
            order_uid,
            kind: dto::fee_policy::FeePolicyKind::PriceImprovement,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: None,
            price_improvement_factor: Some(0.1),
            price_improvement_max_volume_factor: Some(0.99999),
        };
        let expected = vec![fee_policy_1, fee_policy_2, fee_policy_3, fee_policy_4];

        let output = fetch(&mut db, 1, order_uid).await.unwrap();
        assert_eq!(output, expected);
    }
}
