use {
    crate::{auction::AuctionId, Address, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
    std::collections::HashMap,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeAsset {
    pub amount: BigDecimal,
    pub token: Address,
}

pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    block_number: i64,
    executed_fee: &BigDecimal,
    executed_protocol_fees: &[FeeAsset],
) -> Result<(), sqlx::Error> {
    let (protocol_fee_tokens, protocol_fee_amounts) = executed_protocol_fees.iter().fold(
        (
            Vec::with_capacity(executed_protocol_fees.len()),
            Vec::with_capacity(executed_protocol_fees.len()),
        ),
        |(mut tokens, mut amounts), fee| {
            tokens.push(fee.token);
            amounts.push(fee.amount.clone());
            (tokens, amounts)
        },
    );

    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, surplus_fee, block_number, protocol_fee_tokens, protocol_fee_amounts)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (order_uid, auction_id)
DO UPDATE SET reward = $3, surplus_fee = $4, block_number = $5, protocol_fee_tokens = $6, protocol_fee_amounts = $7
;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(Some(executed_fee))
        .bind(block_number)
        .bind(protocol_fee_tokens)
        .bind(protocol_fee_amounts)
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

    let mut query_builder = QueryBuilder::new(
        "SELECT order_uid, auction_id, protocol_fee_tokens, protocol_fee_amounts FROM \
         order_execution WHERE ",
    );

    for (i, (auction_id, order_uid)) in keys_filter.iter().enumerate() {
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

    #[derive(Clone, Debug, Eq, PartialEq, sqlx::Type, sqlx::FromRow)]
    struct ProtocolFees {
        pub order_uid: OrderUid,
        pub auction_id: AuctionId,
        pub protocol_fee_tokens: Vec<Address>,
        pub protocol_fee_amounts: Vec<BigDecimal>,
    }
    let query = query_builder.build_query_as::<ProtocolFees>();
    let rows: Vec<ProtocolFees> = query.fetch_all(ex).await?;

    let mut fees = HashMap::new();
    for row in rows {
        fees.insert(
            (row.auction_id, row.order_uid),
            row.protocol_fee_tokens
                .into_iter()
                .zip(row.protocol_fee_amounts)
                .map(|(token, amount)| FeeAsset { token, amount })
                .collect(),
        );
    }

    Ok(fees)
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

        // save entry with protocol fees
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

        // save entry without protocol fees (simulate case when we are still not
        // calculating them)
        save(&mut db, &Default::default(), 2, 0, &Default::default(), &[])
            .await
            .unwrap();

        let keys: Vec<(AuctionId, OrderUid)> = vec![
            (1, Default::default()),
            (2, Default::default()),
            (3, Default::default()),
        ];

        let protocol_fees = executed_protocol_fees(&mut db, &keys).await.unwrap();
        assert_eq!(protocol_fees.len(), 2);
    }
}
