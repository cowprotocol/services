use {
    crate::{Address, OrderUid, TransactionHash, auction::AuctionId, events::EventIndex},
    bigdecimal::BigDecimal,
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
    /// This trade's share of the settlement transaction's gas cost in native
    /// token wei (`gas_used * effective_gas_price / trades_in_settlement`).
    /// `NULL` for settlements observed before gas was persisted (see V115).
    pub gas_cost: Option<BigDecimal>,
}

/// SQL expression computing a single trade's share of its settlement's on-chain
/// gas cost: the settlement's total cost (`gas_used * effective_gas_price`)
/// divided equally across all trades settled in the same transaction. Expects
/// the settlement row to be aliased `s`; selected as the column `gas_cost`,
/// which is `NULL` for settlements whose gas was not persisted (see migration
/// V115). Shared by [`trades`] and [`ORDER_GAS_COST`] so they cannot drift.
pub(crate) const GAS_COST_EXPR: &str = r#"FLOOR(
            (s.gas_used * s.effective_gas_price)
            / NULLIF((
                SELECT COUNT(*)
                FROM trades tc
                WHERE tc.block_number = s.block_number
                AND   tc.log_index < s.log_index
                AND   tc.log_index > COALESCE((
                    SELECT MAX(sp.log_index)
                    FROM settlements sp
                    WHERE sp.block_number = s.block_number
                    AND   sp.log_index < s.log_index
                ), -1)
            ), 0)
        ) AS gas_cost"#;

/// Scalar subquery yielding a single order's total on-chain gas cost (native
/// token wei) summed across all of its fills, or `NULL` when none of its
/// settlements have persisted gas (see V115). Correlates on the order alias
/// `o`, so it can be embedded in the `orders`/`jit_orders` order-detail queries
/// to fetch the gas cost in the same round-trip. Reuses [`GAS_COST_EXPR`].
pub(crate) const ORDER_GAS_COST: &str = const_format::concatcp!(
    r#"(
    SELECT FLOOR(SUM(settlement.gas_cost))
    FROM trades t
    JOIN LATERAL (
        SELECT "#,
    GAS_COST_EXPR,
    r#"
        FROM settlements s
        WHERE s.block_number = t.block_number
        AND   s.log_index > t.log_index
        ORDER BY s.log_index ASC
        LIMIT 1
    ) AS settlement ON true
    WHERE settlement.gas_cost IS NOT NULL
    AND   t.order_uid = o.uid
)"#,
);

/// [`ORDER_GAS_COST`] rendered as a trailing select-list column named
/// `gas_cost` (comma-prefixed), ready to splice onto an order-detail `SELECT`
/// clause that exposes the order alias `o`. Kept out of the shared `SELECT`
/// fragments so only the queries that need the gas cost pay for it.
pub(crate) const ORDER_GAS_COST_COLUMN: &str =
    const_format::concatcp!(", ", ORDER_GAS_COST, " AS gas_cost");

pub fn trades<'a>(
    ex: &'a mut PgConnection,
    owner_filter: Option<&'a Address>,
    order_uid_filter: Option<&'a OrderUid>,
    offset: i64,
    limit: i64,
) -> instrument::Instrumented<impl Future<Output = Result<Vec<TradesQueryRow>, sqlx::Error>>> {
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
    o.sell_token"#;

    // Resolves the settlement that included each returned trade (the first
    // settlement event in the same block after the trade) and computes that
    // trade's share of the settlement's on-chain gas cost. Joined onto the
    // already-paginated `page` CTE rather than inside the UNION branches, so the
    // settlement lookup and the (relatively expensive) gas computation run only
    // for the rows actually returned instead of for every candidate row across
    // all three branches.
    const GAS_JOIN: &str = const_format::concatcp!(
        r#"
LEFT OUTER JOIN LATERAL (
    SELECT
        s.tx_hash,
        s.auction_id,
        "#,
        GAS_COST_EXPR,
        r#"
    FROM settlements s
    WHERE s.block_number = page.block_number
    AND   s.log_index > page.log_index
    ORDER BY s.log_index ASC
    LIMIT 1
) AS settlement ON true"#,
    );

    const QUERY: &str = const_format::concatcp!(
        "WITH page AS (",
        "(",
        SELECT,
        " FROM trades t",
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
        " JOIN orders o ON o.uid = t.order_uid",
        " JOIN onchain_placed_orders onchain_o",
        " ON onchain_o.uid = t.order_uid",
        " WHERE ($1 IS NULL OR onchain_o.sender = $1)",
        " AND ($2 IS NULL OR t.order_uid = $2)",
        " ORDER BY t.block_number DESC, t.log_index DESC",
        " LIMIT $3 + $4",
        ")",
        " UNION ",
        // Note that we apply 2 tricks here:
        // 1. we invert the join order (join `trades` onto `jit_orders` instead
        // of `jit_orders` onto `trades`). For cases where 1 account has MANY
        // trades joining `jit_orders` onto the trades means fetching data for
        // MANY `jit_orders`. But given that `jit_orders` are rare inverting the
        // join order means we only fetch few or no `jit_orders` at all when
        // looking them up by `owner`.
        // 2. we explicitly use a MATERIALIZED CTE to force the query planner
        // to follow this lookup order. Without using `MATERIALIZED` the query
        // planner can "inline" this sub-query and which can lead to incorrect
        // optimization decisions.
        // Specifically NOT using `MATERIALIZED` can lead to the query
        // planner doing full scans on the `trades` table instead of searching
        // via the `owner` index on the `jit_orders` table.
        "(",
        " WITH jit AS MATERIALIZED (",
        "   SELECT uid, owner, buy_token, sell_token",
        "   FROM jit_orders",
        "   WHERE ($1 IS NULL OR owner = $1)",
        "   AND ($2 IS NULL OR uid = $2)",
        ")",
        SELECT,
        " FROM jit o",
        " JOIN trades t ON o.uid = t.order_uid",
        " ORDER BY t.block_number DESC, t.log_index DESC",
        " LIMIT $3 + $4",
        ")",
        " ORDER BY block_number DESC, log_index DESC",
        " LIMIT $3",
        " OFFSET $4",
        ")",
        r#"
SELECT
    page.block_number,
    page.log_index,
    page.order_uid,
    page.buy_amount,
    page.sell_amount,
    page.sell_amount_before_fees,
    page.owner,
    page.buy_token,
    page.sell_token,
    settlement.tx_hash,
    settlement.auction_id,
    settlement.gas_cost
FROM page"#,
        GAS_JOIN,
        " ORDER BY page.block_number DESC, page.log_index DESC",
    );

    sqlx::query_as(QUERY)
        .bind(owner_filter)
        .bind(order_uid_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(ex)
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
        bigdecimal::ToPrimitive,
        sqlx::Connection,
    };

    /// Generates 1 unique user and the provided number of unique orders
    /// for that user.
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
    async fn postgres_gas_cost_attribution() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // 1 user with 4 orders.
        let mut users_and_orders = generate_owners_and_order_ids(&[4]).await;
        let (owner, orders) = users_and_orders
            .pop()
            .expect("users_and_orders should have 1 element");
        let order_a = orders[0];
        let order_b = orders[1];
        let order_c = orders[2];
        let order_d = orders[3];

        let index = |block: i64, log: i64| EventIndex {
            block_number: block,
            log_index: log,
        };

        // Block 0 holds two settlements in different transactions. Trades before
        // each settlement event belong to it.
        //
        //   log 0: trade (order_a)  ┐
        //   log 1: trade (order_b)  ┴─ settled by A
        //   log 2: settlement A     -> gas_used * price = 100 * 10 = 1000
        //   log 3: trade (order_a)  ┐
        //   log 4: trade (order_c)  ┴─ settled by B
        //   log 5: settlement B     -> gas_used * price = 300 * 10 = 3000
        add_order_and_trade(&mut db, owner, order_a, index(0, 0), None, None).await;
        add_order_and_trade(&mut db, owner, order_b, index(0, 1), None, None).await;
        let settlement_a = add_settlement(
            &mut db,
            index(0, 2),
            Default::default(),
            ByteArray([1; 32]),
            1,
        )
        .await;
        crate::settlements::update_settlement_gas(
            &mut db,
            0,
            2,
            BigDecimal::from(100),
            BigDecimal::from(10),
        )
        .await
        .unwrap();
        // order_a fills a second time, in settlement B.
        add_trade(&mut db, owner, order_a, index(0, 3), None, None).await;
        add_order_and_trade(&mut db, owner, order_c, index(0, 4), None, None).await;
        let settlement_b = add_settlement(
            &mut db,
            index(0, 5),
            Default::default(),
            ByteArray([2; 32]),
            2,
        )
        .await;
        crate::settlements::update_settlement_gas(
            &mut db,
            0,
            5,
            BigDecimal::from(300),
            BigDecimal::from(10),
        )
        .await
        .unwrap();

        // A settlement whose gas was never recorded (e.g. observed before V115)
        // contributes no gas cost.
        add_order_and_trade(&mut db, owner, order_d, index(1, 0), None, None).await;
        let settlement_c = add_settlement(
            &mut db,
            index(1, 1),
            Default::default(),
            ByteArray([3; 32]),
            3,
        )
        .await;

        // Each trade gets an equal share of its settlement's gas cost.
        let mut rows = trades(&mut db, None, None, 0, 1000)
            .into_inner()
            .await
            .unwrap();
        rows.sort_by_key(|row| (row.block_number, row.log_index));
        let gas = |row: &TradesQueryRow| row.gas_cost.as_ref().and_then(|cost| cost.to_u64());

        assert_eq!(rows.len(), 5);
        // Settlement A: 1000 / 2 trades = 500 each.
        assert_eq!(rows[0].order_uid, order_a);
        assert_eq!(gas(&rows[0]), Some(500));
        assert_eq!(rows[0].tx_hash, Some(settlement_a.transaction_hash));
        assert_eq!(rows[1].order_uid, order_b);
        assert_eq!(gas(&rows[1]), Some(500));
        assert_eq!(rows[1].tx_hash, Some(settlement_a.transaction_hash));
        // Settlement B: 3000 / 2 trades = 1500 each.
        assert_eq!(rows[2].order_uid, order_a);
        assert_eq!(gas(&rows[2]), Some(1500));
        assert_eq!(rows[2].tx_hash, Some(settlement_b.transaction_hash));
        assert_eq!(rows[3].order_uid, order_c);
        assert_eq!(gas(&rows[3]), Some(1500));
        assert_eq!(rows[3].tx_hash, Some(settlement_b.transaction_hash));
        // Settlement C: gas not recorded -> no share, but still resolves its tx.
        assert_eq!(rows[4].order_uid, order_d);
        assert_eq!(gas(&rows[4]), None);
        assert_eq!(rows[4].tx_hash, Some(settlement_c.transaction_hash));

        // The order-detail query attributes the same cost per order, summed
        // across the order's fills (see `crate::trades::ORDER_GAS_COST`).
        async fn order_gas(ex: &mut PgConnection, uid: &OrderUid) -> Option<u64> {
            crate::orders::single_full_order_with_quote(ex, uid)
                .await
                .unwrap()
                .unwrap()
                .full_order
                .gas_cost
                .and_then(|cost| cost.to_u64())
        }
        assert_eq!(order_gas(&mut db, &order_a).await, Some(2000)); // 500 + 1500
        assert_eq!(order_gas(&mut db, &order_b).await, Some(500));
        assert_eq!(order_gas(&mut db, &order_c).await, Some(1500));
        // order_d's only settlement has no recorded gas.
        assert_eq!(order_gas(&mut db, &order_d).await, None);
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
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], expected_trades[0]); // block 4
        assert_eq!(result[1], expected_trades[1]); // block 3

        // Test offset: skip first 2, get next 2 (blocks 2 and 1 in DESC order)
        let result = trades(&mut db, Some(&owner), None, 2, 2)
            .into_inner()
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], expected_trades[2]); // block 2
        assert_eq!(result[1], expected_trades[3]); // block 1

        // Test offset beyond available trades
        let result = trades(&mut db, Some(&owner), None, 10, 2)
            .into_inner()
            .await
            .unwrap();
        assert_eq!(result.len(), 0);

        // Test large limit returns all available trades in DESC order
        let result = trades(&mut db, Some(&owner), None, 0, 100)
            .into_inner()
            .await
            .unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result, expected_trades);
    }
}
