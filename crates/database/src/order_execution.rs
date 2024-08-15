use {
    crate::{auction::AuctionId, Address, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::{
        postgres::{PgHasArrayType, PgTypeInfo},
        PgConnection,
    },
};

#[derive(Clone, Debug, Eq, PartialEq, sqlx::Type, sqlx::FromRow)]
pub struct FeeAsset {
    pub amount: BigDecimal,
    pub token: Address,
}

// explains how to store array of FeeAsset in Postgres and how to fetch it
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

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    pub async fn fetch_protocol_fees(
        ex: &mut PgConnection,
        order: &OrderUid,
        auction: AuctionId,
    ) -> Result<Option<Vec<FeeAsset>>, sqlx::Error> {
        const QUERY: &str = r#"
            SELECT protocol_fees
            FROM order_execution
            WHERE order_uid = $1 AND auction_id = $2
        "#;

        let row = sqlx::query_scalar(QUERY)
            .bind(order)
            .bind(auction)
            .fetch_optional(ex)
            .await?;

        Ok(row)
    }

    #[tokio::test]
    //#[ignore]
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

        let protocol_fees = fetch_protocol_fees(&mut db, &Default::default(), 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(protocol_fees.len(), 2);
    }
}
