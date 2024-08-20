use {
    crate::{auction::AuctionId, Address, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::{
        postgres::{PgHasArrayType, PgTypeInfo},
        PgConnection,
        QueryBuilder,
    },
    std::collections::HashMap,
};

#[derive(Clone, Debug, Eq, PartialEq, sqlx::Type, sqlx::FromRow)]
pub struct ProtocolFees {
    pub order_uid: OrderUid,
    pub auction_id: AuctionId,
    pub protocol_fees: Vec<FeeAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq, sqlx::Type, sqlx::FromRow)]
pub struct FeeAsset {
    pub amount: BigDecimal,
    pub token: Address,
}

// explains that the equivalent Postgres type is already defined in the database
impl PgHasArrayType for FeeAsset {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("_feeasset")
    }
}

pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    block_number: i64,
    executed_fee: &BigDecimal,
    executed_protocol_fees: &[FeeAsset],
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, surplus_fee, block_number, protocol_fees)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT (order_uid, auction_id)
DO UPDATE SET reward = $3, surplus_fee = $4, block_number = $5, protocol_fees = $6
;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(Some(executed_fee))
        .bind(block_number)
        .bind(executed_protocol_fees)
        .execute(ex)
        .await?;
    Ok(())
}

// fetch protocol fees for all keys in the filter
pub async fn executed_protocol_fees(
    ex: &mut PgConnection,
    keys_filter: &[(AuctionId, OrderUid)],
) -> Result<HashMap<(AuctionId, OrderUid), Vec<FeeAsset>>, sqlx::Error> {
    if keys_filter.is_empty() {
        return Ok(HashMap::new());
    }

    let mut fees = HashMap::new();
    for fee in get_protocol_fees(ex, keys_filter).await? {
        fees.insert((fee.auction_id, fee.order_uid), fee.protocol_fees);
    }

    Ok(fees)
}

// executed procotol fee for a single <order, auction> pair
async fn get_protocol_fees(
    ex: &mut PgConnection,
    keys: &[(AuctionId, OrderUid)],
) -> Result<Vec<ProtocolFees>, sqlx::Error> {
    if keys.is_empty() {
        return Ok(vec![]);
    }

    let mut query_builder = QueryBuilder::new(
        "SELECT order_uid, auction_id, protocol_fees FROM order_execution WHERE ",
    );

    for (i, (auction_id, order_uid)) in keys.iter().enumerate() {
        if i > 0 {
            query_builder.push(" OR ");
        }
        query_builder
            .push("(order_uid = ")
            .push_bind(order_uid)
            .push(" AND auction_id = ")
            .push_bind(auction_id)
            .push(")");
    }

    let query = query_builder.build_query_as::<ProtocolFees>();
    let rows = query.fetch_all(ex).await?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_save() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        save(
            &mut db,
            &Default::default(),
            1,
            0,
            &Default::default(),
            &[
                FeeAsset {
                    amount: Default::default(),
                    token: Default::default(),
                },
                FeeAsset {
                    amount: Default::default(),
                    token: Default::default(),
                },
            ],
        )
        .await
        .unwrap();

        // save entry but without protocol fees (we are still not calculating them)
        save(&mut db, &Default::default(), 2, 0, &Default::default(), &[])
            .await
            .unwrap();

        let keys: Vec<(AuctionId, OrderUid)> = vec![
            (1, Default::default()),
            (2, Default::default()),
            (3, Default::default()),
        ];

        let protocol_fees = get_protocol_fees(&mut db, &keys).await.unwrap();
        assert_eq!(protocol_fees.len(), 2);
    }
}
