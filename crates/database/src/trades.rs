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
    /// `NULL` for settlements observed before gas was persisted (see V116).
    pub gas_cost: Option<BigDecimal>,
}

/// `LEFT OUTER JOIN LATERAL` clause resolving, for each row of a `trades t`
/// alias, the settlement that included the trade and the trade's share of its
/// on-chain gas cost:
///
/// - A trade belongs to the first settlement following it in the same block.
/// - A settlement's cost is split equally across the trades between it and the
///   previous settlement of the block.
/// - `gas_cost` is `NULL` for settlements whose gas was not persisted (see
///   migration V116).
pub(crate) const SETTLEMENT_JOIN: &str = r#"
LEFT OUTER JOIN LATERAL (
    SELECT
        s.tx_hash,
        s.auction_id,
        FLOOR(
            (s.gas_used * s.effective_gas_price)
            / NULLIF((
                SELECT COUNT(*)
                FROM trades settled
                WHERE settled.block_number = s.block_number
                AND   settled.log_index < s.log_index
                AND   settled.log_index > COALESCE((
                    SELECT MAX(previous.log_index)
                    FROM settlements previous
                    WHERE previous.block_number = s.block_number
                    AND   previous.log_index < s.log_index
                ), -1)
            ), 0)
        ) AS gas_cost
    FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
    ORDER BY s.log_index ASC
    LIMIT 1
) AS settlement ON true"#;

/// Select-list expression computing the gas cost of the order alias `o`
/// summed across all of its fills, as `gas_cost`. Embedded in
/// [`crate::orders::SELECT`] and [`crate::jit_orders::SELECT`].
///
/// The sum is `NULL` unless *every* fill's cost is known — `SUM` alone would
/// skip the unknown ones and report a total that looks complete but
/// understates the order (see the `CASE`).
pub(crate) const ORDER_GAS_COST: &str = const_format::concatcp!(
    ", (SELECT CASE WHEN COUNT(*) = COUNT(settlement.gas_cost) THEN SUM(settlement.gas_cost) END \
     FROM trades t",
    SETTLEMENT_JOIN,
    " WHERE t.order_uid = o.uid) AS gas_cost",
);

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
        " SELECT t.*, settlement.tx_hash, settlement.auction_id, settlement.gas_cost",
        " FROM page t",
        SETTLEMENT_JOIN,
        " ORDER BY t.block_number DESC, t.log_index DESC",
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
    const GAS_PRICE: u64 = 10;

    /// `gas_used` is recorded at [`GAS_PRICE`]; `None` leaves the gas
    /// unrecorded, as for settlements observed before V116.
    async fn add_settlement(
        ex: &mut PgTransaction<'_>,
        event_index: EventIndex,
        solver: Address,
        transaction_hash: TransactionHash,
        auction_id: AuctionId,
        gas_used: Option<u64>,
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
        if let Some(gas_used) = gas_used {
            crate::settlements::update_settlement_solver_and_gas(
                ex,
                event_index.block_number,
                event_index.log_index,
                solver,
                auction_id,
                BigDecimal::from(gas_used),
                BigDecimal::from(GAS_PRICE),
            )
            .await
            .unwrap();
        }
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
            None,
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
            None,
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
        // Only settlement_a's gas is recorded, so the two trades cover both an
        // attributed cost and one that stays unknown (as for settlements
        // observed before V116).
        let settlement_a = add_settlement(
            &mut db,
            settlement_a_event,
            Default::default(),
            Default::default(),
            1,
            Some(100),
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
            None,
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
        // settlement_a settled this trade alone, so it takes the whole cost.
        let trade_a = TradesQueryRow {
            gas_cost: Some(BigDecimal::from(100 * GAS_PRICE)),
            ..trade_a
        };
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

    /// Builds one block of trades and settlements from `trades_per_settlement`:
    /// each element is one settlement given as (number of trades it settled,
    /// its recorded `gas_used`).
    ///
    /// Trade `k` of *every* settlement belongs to `orders[k]`, so an order that
    /// appears in several settlements models a partially fillable order filled
    /// repeatedly. `orders` holds the largest count's worth of orders, so a
    /// settlement with more trades than its predecessors introduces orders
    /// unique to it.
    ///
    /// For `&[(2, 120), (3, 230)]` that lays out:
    ///
    /// ```text
    /// log 0: trade (orders[0])  ┐
    /// log 1: trade (orders[1])  ┴─ settled by settlements[0], gas_used 120
    /// log 2: settlement
    /// log 3: trade (orders[0])  ┐
    /// log 4: trade (orders[1])  │  settled by settlements[1], gas_used 230
    /// log 5: trade (orders[2])  ┘
    /// log 6: settlement
    /// ```
    async fn setup_gas_costs(
        db: &mut PgTransaction<'_>,
        trades_per_settlement: &[(usize, u64)],
    ) -> (Address, Vec<OrderUid>, Vec<Settlement>) {
        let order_count = trades_per_settlement
            .iter()
            .map(|&(trades, _)| trades)
            .max()
            .unwrap_or(0);
        let mut users_and_orders = generate_owners_and_order_ids(&[order_count]).await;
        let (owner, orders) = users_and_orders
            .pop()
            .expect("users_and_orders should have 1 element");

        let mut settlements = Vec::with_capacity(trades_per_settlement.len());
        let mut log_index = 0;
        let mut placed = vec![false; order_count];
        for (index, &(trades, gas_used)) in trades_per_settlement.iter().enumerate() {
            let auction_id = i64::try_from(index).unwrap() + 1;
            for order in 0..trades {
                let event = EventIndex {
                    block_number: 0,
                    log_index,
                };
                // The order row must be inserted once, on the order's first fill.
                if std::mem::replace(&mut placed[order], true) {
                    add_trade(db, owner, orders[order], event, None, None).await;
                } else {
                    add_order_and_trade(db, owner, orders[order], event, None, None).await;
                }
                log_index += 1;
            }
            let settlement = add_settlement(
                db,
                EventIndex {
                    block_number: 0,
                    log_index,
                },
                Default::default(),
                ByteArray([u8::try_from(index).unwrap() + 1; 32]),
                auction_id,
                Some(gas_used),
            )
            .await;
            settlements.push(settlement);
            log_index += 1;
        }

        (owner, orders, settlements)
    }

    /// A settlement's cost is split equally across the trades it settled, and
    /// each trade resolves the transaction that settled it.
    #[tokio::test]
    #[ignore]
    async fn postgres_gas_cost_split_across_trades_of_a_settlement() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // Distinct trade counts and gas so no two settlements attribute the
        // same share, and the middle one divides unevenly. The last settlement
        // has a trade for an order the earlier two lack.
        let (owner, orders, settlements) =
            setup_gas_costs(&mut db, &[(2, 120), (3, 230), (4, 340)]).await;

        let mut rows = trades(&mut db, None, None, 0, 1000)
            .into_inner()
            .await
            .unwrap();
        rows.sort_by_key(|row| (row.block_number, row.log_index));

        // gas_used 120, 230 and 340 at price 10, split 2, 3 and 4 ways. 2300 /
        // 3 does not divide evenly and the share has to be a whole number of
        // wei (which the exact `BigDecimal` equality pins), or
        // `big_decimal_to_u256` rejects it further up and the API silently
        // reports no gas cost at all.
        let row = |order: usize, settlement: usize, share: u64| {
            (
                orders[order],
                Some(BigDecimal::from(share)),
                Some(settlements[settlement].transaction_hash),
            )
        };
        assert_eq!(
            rows.iter()
                .map(|r| (r.order_uid, r.gas_cost.clone(), r.tx_hash))
                .collect::<Vec<_>>(),
            vec![
                row(0, 0, 600),
                row(1, 0, 600),
                row(0, 1, 766),
                row(1, 1, 766),
                row(2, 1, 766),
                row(0, 2, 850),
                row(1, 2, 850),
                row(2, 2, 850),
                row(3, 2, 850),
            ]
        );

        // The divisor counts the settlement's own trades, so restricting what the
        // query returns leaves each share untouched. Dividing by the number of
        // returned rows would hand these two the whole settlement's cost.
        let filtered = trades(&mut db, None, Some(&orders[2]), 0, 1000)
            .into_inner()
            .await
            .unwrap();
        assert_eq!(
            filtered
                .iter()
                .map(|row| row.gas_cost.clone())
                .collect::<Vec<_>>(),
            vec![Some(BigDecimal::from(850)), Some(BigDecimal::from(766))]
        );

        // Nor does paging: `rows` is ascending, so the second row of a
        // descending page is the second from its end.
        let paginated = trades(&mut db, Some(&owner), None, 1, 1)
            .into_inner()
            .await
            .unwrap();
        assert_eq!(paginated.len(), 1);
        assert_eq!(paginated[0], rows[rows.len() - 2]);
    }

    /// Only settlements from the trade's own block can settle it, and only that
    /// settlement's own block counts towards the divisor.
    #[tokio::test]
    #[ignore]
    async fn postgres_gas_cost_ignores_settlements_of_other_blocks() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let mut users_and_orders = generate_owners_and_order_ids(&[2]).await;
        let (owner, orders) = users_and_orders.pop().unwrap();
        let event = |block_number, log_index| EventIndex {
            block_number,
            log_index,
        };

        // Block 0: two trades settled by one settlement, 100 gas -> 500 each.
        add_order_and_trade(&mut db, owner, orders[0], event(0, 0), None, None).await;
        add_order_and_trade(&mut db, owner, orders[1], event(0, 1), None, None).await;
        let first = add_settlement(
            &mut db,
            event(0, 2),
            Default::default(),
            ByteArray([1; 32]),
            1,
            Some(100),
        )
        .await;

        // Block 1: a single trade at a *lower* log index than block 0's
        // settlement, so ignoring `block_number` would attribute it to `first`
        // and divide by the wrong block's trade count.
        add_trade(&mut db, owner, orders[0], event(1, 0), None, None).await;
        let second = add_settlement(
            &mut db,
            event(1, 1),
            Default::default(),
            ByteArray([2; 32]),
            2,
            Some(700),
        )
        .await;

        let mut rows = trades(&mut db, None, None, 0, 1000)
            .into_inner()
            .await
            .unwrap();
        rows.sort_by_key(|row| (row.block_number, row.log_index));
        assert_eq!(
            rows.iter()
                .map(|row| (row.gas_cost.clone(), row.tx_hash))
                .collect::<Vec<_>>(),
            vec![
                (Some(BigDecimal::from(500)), Some(first.transaction_hash)),
                (Some(BigDecimal::from(500)), Some(first.transaction_hash)),
                // Block 1's sole trade takes all of its own settlement's cost.
                (Some(BigDecimal::from(7000)), Some(second.transaction_hash)),
            ]
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
