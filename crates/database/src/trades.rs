use {
    crate::{Address, OrderUid, TransactionHash, auction::AuctionId, events::EventIndex},
    bigdecimal::BigDecimal,
    futures::stream::BoxStream,
    sqlx::PgConnection,
    tracing::{Instrument, info_span, instrument},
};

#[derive(Clone, Debug, Default, Eq, PartialEq, sqlx::FromRow)]
pub struct TradesQueryRow {
    pub block_number: i64,
    pub log_index: i64,
    pub order_uid: OrderUid,
    pub buy_amount: BigDecimal,
    pub sell_amount: BigDecimal,
    pub sell_amount_before_fees: BigDecimal,
    pub owner: Address,
    pub buy_token: Address,
    pub sell_token: Address,
    pub tx_hash: Option<TransactionHash>,
    pub auction_id: Option<AuctionId>,
}

pub fn trades<'a>(
    ex: &'a mut PgConnection,
    owner_filter: Option<&'a Address>,
    order_uid_filter: Option<&'a OrderUid>,
    offset: i64,
    limit: i64,
) -> instrument::Instrumented<BoxStream<'a, Result<TradesQueryRow, sqlx::Error>>> {
    const SELECT: &str = r#"
SELECT
    t.block_number,
    t.log_index,
    t.order_uid,
    t.buy_amount,
    t.sell_amount,
    t.sell_amount - t.fee_amount as sell_amount_before_fees,
    o.owner,
    o.buy_token,
    o.sell_token,
    settlement.tx_hash,
    settlement.auction_id"#;

    const SETTLEMENT_JOIN: &str = r#"
LEFT OUTER JOIN LATERAL (
    SELECT tx_hash, auction_id FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
    ORDER BY s.log_index ASC
    LIMIT 1
) AS settlement ON true"#;

    const QUERY: &str = const_format::concatcp!(
        "(",
        SELECT,
        " FROM trades t",
        SETTLEMENT_JOIN,
        " JOIN orders o ON o.uid = t.order_uid",
        // the uid already contains the owner address and we have
        // an index on this expression so this is very efficient
        " WHERE ($1 IS NULL OR substring(t.order_uid, 33, 20) = $1)",
        " AND ($2 IS NULL OR t.order_uid = $2)",
        " ORDER BY t.block_number DESC, t.log_index DESC",
        " LIMIT $3 + $4",
        ")",
        " UNION ",
        "(",
        SELECT,
        " FROM trades t",
        SETTLEMENT_JOIN,
        " JOIN orders o ON o.uid = t.order_uid",
        " JOIN onchain_placed_orders onchain_o",
        " ON onchain_o.uid = t.order_uid",
        " WHERE ($1 IS NULL OR onchain_o.sender = $1)",
        " AND ($2 IS NULL OR t.order_uid = $2)",
        " ORDER BY t.block_number DESC, t.log_index DESC",
        " LIMIT $3 + $4",
        ")",
        " UNION ",
        "(",
        SELECT,
        // note that we invert the join order here because there are
        // very few jit orders so for accounts with many trades
        // it's a lot more efficient to fetch all jit orders and join
        // trades on top than fetch all trades and then join the jit
        // orders on top.
        " FROM jit_orders o",
        " JOIN trades t ON o.uid = t.order_uid",
        SETTLEMENT_JOIN,
        " WHERE ($1 IS NULL OR o.owner = $1)",
        " AND ($2 IS NULL OR o.uid = $2)",
        " ORDER BY t.block_number DESC, t.log_index DESC",
        " LIMIT $3 + $4",
        ")",
        " ORDER BY block_number DESC, log_index DESC",
        " LIMIT $3",
        " OFFSET $4",
    );

    sqlx::query_as(QUERY)
        .bind(owner_filter)
        .bind(order_uid_filter)
        .bind(limit)
        .bind(offset)
        .fetch(ex)
        .instrument(info_span!("trades"))
}

#[derive(Clone, Debug, Default, Eq, PartialEq, sqlx::FromRow)]
pub struct TradeEvent {
    pub block_number: i64,
    pub log_index: i64,
    pub order_uid: OrderUid,
}

#[instrument(skip_all)]
pub async fn get_trades_for_settlement(
    ex: &mut PgConnection,
    settlement: EventIndex,
) -> Result<Vec<TradeEvent>, sqlx::Error> {
    const QUERY: &str = r#"
WITH
    -- The log index in this query is the log index of the settlement event from the previous (lower log index) settlement in the same transaction or 0 if there is no previous settlement.
    previous_settlement AS (
        SELECT COALESCE(MAX(log_index), 0)
        FROM settlements
        WHERE block_number = $1 AND log_index < $2
    )
SELECT
    block_number,
    log_index,
    order_uid
FROM trades t
WHERE t.block_number = $1
AND t.log_index BETWEEN (SELECT * from previous_settlement) AND $2
"#;
    sqlx::query_as(QUERY)
        .bind(settlement.block_number)
        .bind(settlement.log_index)
        .fetch_all(ex)
        .await
}

#[instrument(skip_all)]
pub async fn token_first_trade_block(
    ex: &mut PgConnection,
    token: Address,
) -> Result<Option<i64>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT MIN(sub.block_number) AS earliest_block
FROM (
    SELECT MIN(t.block_number) AS block_number
    FROM trades t
    JOIN orders o ON t.order_uid = o.uid
    WHERE o.sell_token = $1 OR o.buy_token = $1

    UNION ALL

    SELECT MIN(t.block_number) AS block_number
    FROM trades t
    JOIN jit_orders j ON t.order_uid = j.uid
    WHERE j.sell_token = $1 OR j.buy_token = $1
) AS sub
"#;

    let (block_number,) = sqlx::query_as(QUERY).bind(token).fetch_one(ex).await?;
    Ok(block_number)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            PgTransaction,
            byte_array::ByteArray,
            events::{Event, EventIndex, Settlement, Trade},
            onchain_broadcasted_orders::{OnchainOrderPlacement, insert_onchain_order},
            orders::Order,
        },
        futures::TryStreamExt,
        sqlx::Connection,
    };

    // Generates 1 unique user and the provided number of unique orders
    // for that user.
    async fn generate_owners_and_order_ids(
        orders_for_user: &[usize],
    ) -> Vec<(Address, Vec<OrderUid>)> {
        orders_for_user
            .iter()
            .enumerate()
            .map(|(index, num_orders)| {
                let user = ByteArray([index as u8; 20]);
                let orders = (0usize..*num_orders)
                    .map(|index| {
                        let mut uid_bytes = [index as u8; 56];
                        // make sure to write the owner bytes correctly into
                        // the order uid since those are used in some queries
                        uid_bytes[32..52].copy_from_slice(&user.0);
                        ByteArray(uid_bytes)
                    })
                    .collect();
                (user, orders)
            })
            .collect()
    }

    async fn add_trade(
        ex: &mut PgTransaction<'_>,
        owner: Address,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<TransactionHash>,
        auction_id: Option<AuctionId>,
    ) -> TradesQueryRow {
        crate::events::append(
            ex,
            &[(
                event_index,
                Event::Trade(Trade {
                    order_uid: ByteArray(order_uid.0),
                    ..Default::default()
                }),
            )],
        )
        .await
        .unwrap();
        TradesQueryRow {
            block_number: event_index.block_number,
            log_index: event_index.log_index,
            order_uid,
            owner,
            tx_hash,
            auction_id,
            ..Default::default()
        }
    }

    async fn add_order_and_trade(
        ex: &mut PgTransaction<'_>,
        owner: Address,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<TransactionHash>,
        auction_id: Option<AuctionId>,
    ) -> TradesQueryRow {
        let order = Order {
            uid: order_uid,
            owner,
            ..Default::default()
        };
        crate::orders::insert_order(ex, &order).await.unwrap();
        add_trade(ex, owner, order_uid, event_index, tx_hash, auction_id).await
    }

    async fn assert_trades(
        db: &mut PgConnection,
        owner_filter: Option<&Address>,
        order_uid_filter: Option<&OrderUid>,
        expected: &[TradesQueryRow],
    ) {
        // Use large limit to get all trades
        let mut filtered = trades(db, owner_filter, order_uid_filter, 0, 1000)
            .into_inner()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        filtered.sort_by_key(|t| (t.block_number, t.log_index));
        assert_eq!(filtered, expected);
    }

    // Testing trades without corresponding settlement events
    #[tokio::test]
    #[ignore]
    async fn postgres_trades_without_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // 1 user with 2 orders
        let users_and_orders = generate_owners_and_order_ids(&[2]).await;
        assert_trades(&mut db, None, None, &[]).await;
        let event_index_a = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_a = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[0],
            event_index_a,
            None,
            None,
        )
        .await;
        assert_trades(&mut db, None, None, std::slice::from_ref(&trade_a)).await;

        let event_index_b = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let trade_b = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[1],
            event_index_b,
            None,
            None,
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_owner_filter_benchmark_test() {
        // This test can be used for benchmarking. With i in 0..240
        // and j 0..100, the query should be less than 5 ms.
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();
        for i in 0..1u32 {
            let mut owner_bytes = i.to_ne_bytes().to_vec();
            owner_bytes.append(&mut vec![0; 20 - owner_bytes.len()]);
            let owner = ByteArray(owner_bytes.try_into().unwrap());
            for j in 0..1u32 {
                let mut i_as_bytes = i.to_ne_bytes().to_vec();
                let mut j_as_bytes = j.to_ne_bytes().to_vec();
                let mut order_uid_info = vec![0; 56 - i_as_bytes.len() - j_as_bytes.len()];
                order_uid_info.append(&mut j_as_bytes);
                i_as_bytes.append(&mut order_uid_info);
                let event_index_0 = EventIndex {
                    block_number: 0,
                    log_index: 0,
                };
                let order_uid = ByteArray(i_as_bytes.try_into().unwrap());
                insert_onchain_order(
                    &mut db,
                    &event_index_0.clone(),
                    &OnchainOrderPlacement {
                        order_uid,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
                add_order_and_trade(&mut db, owner, order_uid, event_index_0, None, None).await;
            }
        }

        let now = std::time::Instant::now();
        trades(&mut db, Some(&ByteArray([2u8; 20])), None, 0, 100)
            .into_inner()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        let elapsed = now.elapsed();
        println!("{elapsed:?}");
        assert!(elapsed < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_owner_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let users_and_orders = generate_owners_and_order_ids(&[1, 1, 1, 1, 1]).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[0],
            event_index_0,
            None,
            None,
        )
        .await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 = add_order_and_trade(
            &mut db,
            users_and_orders[1].0,
            users_and_orders[1].1[0],
            event_index_1,
            None,
            None,
        )
        .await;

        assert_trades(
            &mut db,
            Some(&users_and_orders[0].0),
            None,
            std::slice::from_ref(&trade_0),
        )
        .await;
        assert_trades(
            &mut db,
            Some(&users_and_orders[1].0),
            None,
            std::slice::from_ref(&trade_1),
        )
        .await;
        assert_trades(&mut db, Some(&users_and_orders[2].0), None, &[]).await;

        let onchain_order = OnchainOrderPlacement {
            order_uid: users_and_orders[3].1[0],
            sender: users_and_orders[4].0,
            placement_error: None,
        };
        let event_index_2 = EventIndex {
            block_number: 0,
            log_index: 2,
        };
        let trade_2 = add_order_and_trade(
            &mut db,
            users_and_orders[3].0,
            users_and_orders[3].1[0],
            event_index_2,
            None,
            None,
        )
        .await;
        insert_onchain_order(&mut db, &event_index_2, &onchain_order)
            .await
            .unwrap();
        assert_trades(
            &mut db,
            Some(&users_and_orders[4].0),
            None,
            std::slice::from_ref(&trade_2),
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_order_uid_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // 3 users with 1 order each
        let users_and_orders = generate_owners_and_order_ids(&[1, 1, 1]).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[0],
            event_index_0,
            None,
            None,
        )
        .await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 = add_order_and_trade(
            &mut db,
            users_and_orders[1].0,
            users_and_orders[1].1[0],
            event_index_1,
            None,
            None,
        )
        .await;

        assert_trades(&mut db, None, Some(&users_and_orders[0].1[0]), &[trade_0]).await;
        assert_trades(&mut db, None, Some(&users_and_orders[1].1[0]), &[trade_1]).await;
        assert_trades(&mut db, None, Some(&users_and_orders[2].1[0]), &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trade_without_matching_order() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // 1 user with 1 order
        let users_and_trades = generate_owners_and_order_ids(&[1]).await;

        let event_index = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        add_trade(
            &mut db,
            users_and_trades[0].0,
            users_and_trades[0].1[0],
            event_index,
            None,
            None,
        )
        .await;
        // Trade exists in DB but no matching order
        assert_trades(&mut db, None, Some(&users_and_trades[0].1[0]), &[]).await;
        assert_trades(&mut db, Some(&users_and_trades[0].0), None, &[]).await;
    }

    // Testing Trades with settlements
    async fn add_settlement(
        ex: &mut PgTransaction<'_>,
        event_index: EventIndex,
        solver: Address,
        transaction_hash: TransactionHash,
        auction_id: AuctionId,
    ) -> Settlement {
        let settlement = Settlement {
            solver,
            transaction_hash,
        };
        crate::events::append(ex, &[(event_index, Event::Settlement(settlement))])
            .await
            .unwrap();
        crate::settlements::update_settlement_auction(
            ex,
            event_index.block_number,
            event_index.log_index,
            auction_id,
        )
        .await
        .unwrap();
        settlement
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_having_same_settlement_with_and_without_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // 1 user with 2 orders
        let users_and_orders = generate_owners_and_order_ids(&[2]).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            Default::default(),
            Default::default(),
            1,
        )
        .await;

        let trade_a = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
            Some(1),
        )
        .await;
        assert_trades(&mut db, None, None, std::slice::from_ref(&trade_a)).await;

        let trade_b = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
            Some(1),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_same_settlement_no_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // 1 user with 2 orders
        let users_and_trades = generate_owners_and_order_ids(&[2]).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            Default::default(),
            Default::default(),
            1,
        )
        .await;

        add_trade(
            &mut db,
            users_and_trades[0].0,
            users_and_trades[0].1[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
            Some(1),
        )
        .await;

        add_trade(
            &mut db,
            users_and_trades[0].0,
            users_and_trades[0].1[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
            Some(1),
        )
        .await;
        // Trades query returns nothing when there are no corresponding orders.
        assert_trades(&mut db, None, None, &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_two_settlements_in_same_block() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // 1 user with 2 orders
        let users_and_orders = generate_owners_and_order_ids(&[2]).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement_a_event = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let settlement_a = add_settlement(
            &mut db,
            settlement_a_event,
            Default::default(),
            Default::default(),
            1,
        )
        .await;

        let settlement_b_event = EventIndex {
            block_number: 0,
            log_index: 3,
        };
        let settlement_b = add_settlement(
            &mut db,
            settlement_b_event,
            Default::default(),
            ByteArray([2; 32]),
            1,
        )
        .await;

        let trade_a = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement_a.transaction_hash),
            Some(1),
        )
        .await;
        assert_trades(&mut db, None, None, std::slice::from_ref(&trade_a)).await;

        let trade_b = add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[1],
            EventIndex {
                block_number: 0,
                log_index: 2,
            },
            Some(settlement_b.transaction_hash),
            Some(1),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a.clone(), trade_b.clone()]).await;

        // make sure that for a settlement_a in the same block, only trade_a is returned
        assert_eq!(
            get_trades_for_settlement(&mut db, settlement_a_event)
                .await
                .unwrap(),
            vec![TradeEvent {
                block_number: 0,
                log_index: 0,
                order_uid: trade_a.order_uid,
            }]
        );

        // make sure that for a settlement_b in the same block, only trade_b is returned
        assert_eq!(
            get_trades_for_settlement(&mut db, settlement_b_event)
                .await
                .unwrap(),
            vec![TradeEvent {
                block_number: 0,
                log_index: 2,
                order_uid: trade_b.order_uid,
            }]
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_token_first_trade_block() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let token = Default::default();
        assert_eq!(token_first_trade_block(&mut db, token).await.unwrap(), None);

        // 2 users with 1 order each
        let users_and_orders = generate_owners_and_order_ids(&[1, 1]).await;
        let event_index_a = EventIndex {
            block_number: 123,
            log_index: 0,
        };
        let event_index_b = EventIndex {
            block_number: 124,
            log_index: 0,
        };
        add_order_and_trade(
            &mut db,
            users_and_orders[0].0,
            users_and_orders[0].1[0],
            event_index_a,
            None,
            None,
        )
        .await;
        add_order_and_trade(
            &mut db,
            users_and_orders[1].0,
            users_and_orders[1].1[0],
            event_index_b,
            None,
            None,
        )
        .await;
        assert_eq!(
            token_first_trade_block(&mut db, token).await.unwrap(),
            Some(123)
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_pagination() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // Create 5 trades with the same owner
        let users_and_orders = generate_owners_and_order_ids(&[5]).await;
        let owner = users_and_orders[0].0;

        let mut expected_trades = Vec::new();
        for (i, order_id) in users_and_orders[0].1.iter().enumerate() {
            let trade = add_order_and_trade(
                &mut db,
                owner,
                *order_id,
                EventIndex {
                    block_number: i.try_into().unwrap(),
                    log_index: 0,
                },
                None,
                None,
            )
            .await;
            expected_trades.push(trade);
        }

        // Sort expected trades by block_number DESC (matching query ORDER BY)
        expected_trades.sort_by_key(|trade| std::cmp::Reverse(trade.block_number));

        // Test limit: get first 2 trades (blocks 4 and 3 in DESC order)
        let result = trades(&mut db, Some(&owner), None, 0, 2)
            .into_inner()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], expected_trades[0]); // block 4
        assert_eq!(result[1], expected_trades[1]); // block 3

        // Test offset: skip first 2, get next 2 (blocks 2 and 1 in DESC order)
        let result = trades(&mut db, Some(&owner), None, 2, 2)
            .into_inner()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], expected_trades[2]); // block 2
        assert_eq!(result[1], expected_trades[3]); // block 1

        // Test offset beyond available trades
        let result = trades(&mut db, Some(&owner), None, 10, 2)
            .into_inner()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(result.len(), 0);

        // Test large limit returns all available trades in DESC order
        let result = trades(&mut db, Some(&owner), None, 0, 100)
            .into_inner()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result, expected_trades);
    }
}
