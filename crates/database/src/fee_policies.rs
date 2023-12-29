use {
    crate::{auction::AuctionId, OrderUid, PgTransaction},
    sqlx::PgConnection,
    std::ops::DerefMut,
};

#[derive(Debug, Clone, PartialEq)]
pub struct FeePolicy {
    pub auction_id: AuctionId,
    pub order_uid: OrderUid,
    pub kind: FeePolicyKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeePolicyKind {
    PriceImprovement {
        price_improvement_factor: f64,
        max_volume_factor: f64,
    },
    Volume {
        factor: f64,
    },
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct FeePolicyRow {
    auction_id: AuctionId,
    order_uid: OrderUid,
    kind: FeePolicyKindRow,
    price_improvement_factor: Option<f64>,
    volume_factor: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "PolicyKind", rename_all = "lowercase")]
enum FeePolicyKindRow {
    PriceImprovement,
    Volume,
}

pub async fn insert(ex: &mut PgTransaction<'_>, fee_policy: FeePolicy) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO fee_policies (auction_id, order_uid, kind, price_improvement_factor, volume_factor)
        VALUES ($1, $2, $3, $4, $5)
    "#;
    let fee_policy = FeePolicyRow::from(fee_policy);
    sqlx::query(QUERY)
        .bind(fee_policy.auction_id)
        .bind(fee_policy.order_uid)
        .bind(fee_policy.kind)
        .bind(fee_policy.price_improvement_factor)
        .bind(fee_policy.volume_factor)
        .execute(ex.deref_mut())
        .await?;
    Ok(())
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
    let rows = sqlx::query_as::<_, FeePolicyRow>(QUERY)
        .bind(auction_id)
        .bind(order_uid)
        .fetch_all(ex)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(rows)
}

impl From<FeePolicyRow> for FeePolicy {
    fn from(row: FeePolicyRow) -> FeePolicy {
        let kind = match row.kind {
            FeePolicyKindRow::PriceImprovement => FeePolicyKind::PriceImprovement {
                price_improvement_factor: row
                    .price_improvement_factor
                    .expect("missing price improvement factor"),
                max_volume_factor: row.volume_factor.expect("missing volume factor"),
            },
            FeePolicyKindRow::Volume => FeePolicyKind::Volume {
                factor: row.volume_factor.expect("missing volume factor"),
            },
        };
        FeePolicy {
            auction_id: row.auction_id,
            order_uid: row.order_uid,
            kind,
        }
    }
}

impl From<FeePolicy> for FeePolicyRow {
    fn from(fee_policy: FeePolicy) -> Self {
        match fee_policy.kind {
            FeePolicyKind::PriceImprovement {
                price_improvement_factor,
                max_volume_factor,
            } => FeePolicyRow {
                auction_id: fee_policy.auction_id,
                order_uid: fee_policy.order_uid,
                kind: FeePolicyKindRow::PriceImprovement,
                price_improvement_factor: Some(price_improvement_factor),
                volume_factor: Some(max_volume_factor),
            },
            FeePolicyKind::Volume { factor } => FeePolicyRow {
                auction_id: fee_policy.auction_id,
                order_uid: fee_policy.order_uid,
                kind: FeePolicyKindRow::Volume,
                price_improvement_factor: None,
                volume_factor: Some(factor),
            },
        }
    }
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
        let fee_policy_1 = FeePolicy {
            auction_id,
            order_uid,
            kind: FeePolicyKind::PriceImprovement {
                price_improvement_factor: 0.1,
                max_volume_factor: 1.0,
            },
        };
        insert(&mut db, fee_policy_1.clone()).await.unwrap();

        // price improvement fee policy with caps
        let fee_policy_2 = FeePolicy {
            auction_id,
            order_uid,
            kind: FeePolicyKind::PriceImprovement {
                price_improvement_factor: 0.2,
                max_volume_factor: 0.05,
            },
        };
        insert(&mut db, fee_policy_2.clone()).await.unwrap();

        // volume based fee policy
        let fee_policy_3 = FeePolicy {
            auction_id,
            order_uid,
            kind: FeePolicyKind::Volume { factor: 0.06 },
        };
        insert(&mut db, fee_policy_3.clone()).await.unwrap();

        let output = fetch(&mut db, 1, order_uid).await.unwrap();
        assert_eq!(output, vec![fee_policy_1, fee_policy_2, fee_policy_3]);
    }
}
