use {
    crate::{Address, OrderUid, auction::AuctionId, dedup_keep_last},
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
    std::collections::HashMap,
    tracing::instrument,
};

type Execution = (AuctionId, OrderUid);

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct OrderExecution {
    pub order_uid: OrderUid,
    pub auction_id: AuctionId,
    pub reward: f64,
    pub executed_fee: BigDecimal,
    pub executed_fee_token: Address,
    pub block_number: i64,
    pub protocol_fee_tokens: Vec<Address>,
    pub protocol_fee_amounts: Vec<BigDecimal>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    pub amount: BigDecimal,
    pub token: Address,
}

/// A single `order_execution` row to be upserted by [`save_batch`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderExecutionRecord {
    pub order_uid: OrderUid,
    pub auction_id: AuctionId,
    pub block_number: i64,
    pub executed_fee: Asset,
    pub protocol_fees: Vec<Asset>,
}

/// Upserts many `order_execution` rows in a single (chunked) statement. A
/// settlement touches one row per traded order, so this collapses N round-trips
/// into one. Equivalent to calling [`save`] for each record in order.
#[instrument(skip_all)]
pub async fn save_batch(
    ex: &mut PgConnection,
    records: &[OrderExecutionRecord],
) -> Result<(), sqlx::Error> {
    // 8 bind parameters per row; stay well under Postgres' 65535 limit.
    const BATCH_SIZE: usize = 5000;
    // Keep the last record per (order_uid, auction_id) so the batched upsert
    // never targets the same row twice in one statement.
    let records = dedup_keep_last(records, |record| (record.order_uid, record.auction_id));
    for chunk in records.chunks(BATCH_SIZE) {
        let mut builder = QueryBuilder::new(
            "INSERT INTO order_execution (order_uid, auction_id, reward, executed_fee, \
             executed_fee_token, block_number, protocol_fee_tokens, protocol_fee_amounts) ",
        );
        builder.push_values(chunk, |mut builder, record| {
            let (protocol_fee_tokens, protocol_fee_amounts): (Vec<_>, Vec<_>) = record
                .protocol_fees
                .iter()
                .map(|entry| (entry.token, entry.amount.clone()))
                .unzip();
            builder
                .push_bind(record.order_uid)
                .push_bind(record.auction_id)
                .push_bind(0.) // reward is deprecated but saved for historical analysis
                .push_bind(record.executed_fee.amount.clone())
                .push_bind(record.executed_fee.token)
                .push_bind(record.block_number)
                .push_bind(protocol_fee_tokens)
                .push_bind(protocol_fee_amounts);
        });
        builder.push(
            " ON CONFLICT (order_uid, auction_id) DO UPDATE SET reward = EXCLUDED.reward, \
             executed_fee = EXCLUDED.executed_fee, executed_fee_token = \
             EXCLUDED.executed_fee_token, block_number = EXCLUDED.block_number, \
             protocol_fee_tokens = EXCLUDED.protocol_fee_tokens, protocol_fee_amounts = \
             EXCLUDED.protocol_fee_amounts",
        );
        builder.build().execute(&mut *ex).await?;
    }
    Ok(())
}

#[instrument(skip_all)]
pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    block_number: i64,
    executed_fee: Asset,
    protocol_fees: &[Asset],
) -> Result<(), sqlx::Error> {
    let (protocol_fee_tokens, protocol_fee_amounts): (Vec<_>, Vec<_>) = protocol_fees
        .iter()
        .map(|entry| (entry.token, entry.amount.clone()))
        .unzip();

    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, executed_fee, executed_fee_token, block_number, protocol_fee_tokens, protocol_fee_amounts)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (order_uid, auction_id)
DO UPDATE SET reward = $3, executed_fee = $4, executed_fee_token = $5, block_number = $6, protocol_fee_tokens = $7, protocol_fee_amounts = $8
;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(executed_fee.amount)
        .bind(executed_fee.token)
        .bind(block_number)
        .bind(protocol_fee_tokens)
        .bind(protocol_fee_amounts)
        .execute(ex)
        .await?;
    Ok(())
}

#[instrument(skip_all)]
pub async fn read_by_order_uid(
    ex: &mut PgConnection,
    order_uid: &OrderUid,
) -> Result<Vec<OrderExecution>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT order_uid, auction_id, reward, executed_fee, executed_fee_token,
       block_number, protocol_fee_tokens, protocol_fee_amounts
FROM order_execution
WHERE order_uid = $1
ORDER BY auction_id
    "#;
    sqlx::query_as(QUERY).bind(order_uid).fetch_all(ex).await
}

/// Fetch protocol fees for all keys in the filter
#[instrument(skip_all)]
pub async fn executed_protocol_fees(
    ex: &mut PgConnection,
    keys_filter: &[Execution],
) -> Result<HashMap<Execution, Vec<Asset>>, sqlx::Error> {
    if keys_filter.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query_builder = QueryBuilder::new(
        "SELECT oe.order_uid, oe.auction_id, oe.protocol_fee_tokens, oe.protocol_fee_amounts FROM \
         order_execution oe INNER JOIN (VALUES ",
    );

    for (i, (auction_id, order_uid)) in keys_filter.iter().enumerate() {
        if i > 0 {
            query_builder.push(", ");
        }
        query_builder
            .push("(")
            .push_bind(order_uid)
            .push(", ")
            .push_bind(auction_id)
            .push(")");
    }

    query_builder.push(") AS vals(order_uid, auction_id) ");
    query_builder.push("ON (oe.order_uid, oe.auction_id) = (vals.order_uid, vals.auction_id)");

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
                .map(|(token, amount)| Asset { token, amount })
                .collect(),
        );
    }

    Ok(fees)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_save() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // save entry with protocol fees
        let protocol_fees = vec![
            Asset {
                amount: BigDecimal::from(1),
                token: Default::default(),
            },
            Asset {
                amount: BigDecimal::from(2),
                token: Default::default(),
            },
        ];
        save(
            &mut db,
            &Default::default(),
            1,
            0,
            Asset {
                amount: BigDecimal::from(3),
                token: Default::default(),
            },
            protocol_fees.as_slice(),
        )
        .await
        .unwrap();

        // save entry for an order without protocol fees
        save(
            &mut db,
            &Default::default(),
            2,
            0,
            Asset {
                amount: BigDecimal::from(3),
                token: Default::default(),
            },
            &[],
        )
        .await
        .unwrap();

        let keys: Vec<(AuctionId, OrderUid)> = vec![
            (1, Default::default()),
            (2, Default::default()),
            (3, Default::default()),
        ];

        let read_protocol_fees = executed_protocol_fees(&mut db, &keys).await.unwrap();
        assert_eq!(read_protocol_fees.len(), 2);
        assert_eq!(read_protocol_fees[&(1, Default::default())], protocol_fees);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_batch() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let uid_a = ByteArray([1; 56]);
        let uid_b = ByteArray([2; 56]);
        let token = ByteArray([3; 20]);

        // Empty input is a no-op.
        save_batch(&mut db, &[]).await.unwrap();

        // Batch two orders (one with protocol fees, one without) plus a duplicate
        // of `uid_a` in the same batch: the last occurrence must win.
        save_batch(
            &mut db,
            &[
                OrderExecutionRecord {
                    order_uid: uid_a,
                    auction_id: 1,
                    block_number: 10,
                    executed_fee: Asset {
                        amount: BigDecimal::from(1),
                        token,
                    },
                    protocol_fees: vec![Asset {
                        amount: BigDecimal::from(2),
                        token,
                    }],
                },
                OrderExecutionRecord {
                    order_uid: uid_b,
                    auction_id: 1,
                    block_number: 11,
                    executed_fee: Asset {
                        amount: BigDecimal::from(3),
                        token,
                    },
                    protocol_fees: vec![],
                },
                OrderExecutionRecord {
                    order_uid: uid_a,
                    auction_id: 1,
                    block_number: 99,
                    executed_fee: Asset {
                        amount: BigDecimal::from(7),
                        token,
                    },
                    protocol_fees: vec![
                        Asset {
                            amount: BigDecimal::from(8),
                            token,
                        },
                        Asset {
                            amount: BigDecimal::from(9),
                            token,
                        },
                    ],
                },
            ],
        )
        .await
        .unwrap();

        let a = read_by_order_uid(&mut db, &uid_a).await.unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].auction_id, 1);
        assert_eq!(a[0].block_number, 99);
        assert_eq!(a[0].executed_fee, BigDecimal::from(7));
        assert_eq!(
            a[0].protocol_fee_amounts,
            vec![BigDecimal::from(8), BigDecimal::from(9)]
        );

        let b = read_by_order_uid(&mut db, &uid_b).await.unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].block_number, 11);
        assert!(b[0].protocol_fee_tokens.is_empty());

        // Re-saving `uid_a` must update it in place (ON CONFLICT DO UPDATE).
        save_batch(
            &mut db,
            &[OrderExecutionRecord {
                order_uid: uid_a,
                auction_id: 1,
                block_number: 100,
                executed_fee: Asset {
                    amount: BigDecimal::from(5),
                    token,
                },
                protocol_fees: vec![],
            }],
        )
        .await
        .unwrap();
        let a = read_by_order_uid(&mut db, &uid_a).await.unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].block_number, 100);
        assert_eq!(a[0].executed_fee, BigDecimal::from(5));
        assert!(a[0].protocol_fee_tokens.is_empty());
    }
}
