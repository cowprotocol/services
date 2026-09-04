use {
    crate::{
        Address,
        OrderUid,
        PgTransaction,
        TransactionHash,
        auction::AuctionId,
        events::EventIndex,
    },
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
    std::ops::DerefMut,
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
    /// Share of the settlement's gas cost in native token wei, as attributed
    /// by [`attribute_gas_cost`]. `NULL` for settlements observed before the
    /// column existed, `0` for a liquidity-only JIT order.
    pub gas_cost: Option<BigDecimal>,
}

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
    o.sell_token,
    t.gas_cost,
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
        SETTLEMENT_JOIN,
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
        .fetch_all(ex)
        .instrument(info_span!("trades"))
}

#[derive(Clone, Debug, Default, Eq, PartialEq, sqlx::FromRow)]
pub struct TradeEvent {
    pub block_number: i64,
    pub log_index: i64,
    pub order_uid: OrderUid,
}

/// A CTE named `settled` holding the trades one settlement settled. Prefix it
/// to a query that binds the settlement's block number to `$1` and its log
/// index to `$2`.
///
/// The lower bound is the log index of the previous (lower log index)
/// settlement in the same block, or 0 if there is no previous settlement.
/// `order_uid` is only needed by the read path.
const SETTLED_TRADES_CTE: &str = r#"
WITH previous_settlement AS (
    SELECT COALESCE(MAX(log_index), 0)
    FROM settlements
    WHERE block_number = $1 AND log_index < $2
),
settled AS (
    SELECT block_number, log_index, order_uid
    FROM trades
    WHERE block_number = $1
    AND log_index BETWEEN (SELECT * from previous_settlement) AND $2
)
"#;

#[instrument(skip_all)]
pub async fn get_trades_for_settlement(
    ex: &mut PgConnection,
    settlement: EventIndex,
) -> Result<Vec<TradeEvent>, sqlx::Error> {
    const QUERY: &str = const_format::concatcp!(
        SETTLED_TRADES_CTE,
        "SELECT block_number, log_index, order_uid FROM settled"
    );
    sqlx::query_as(QUERY)
        .bind(settlement.block_number)
        .bind(settlement.log_index)
        .fetch_all(ex)
        .await
}

/// Splits a settlement's gas cost equally between the user trades it settled,
/// storing each share in `trades.gas_cost`.
///
/// A trade is a user trade if its order is in the `orders` table, or if its
/// owner is in `surplus_capturing_jit_order_owners`. Every other trade settled
/// a JIT order that only provides liquidity for the user trades, so it gets a
/// share of 0 instead of taking one away from them.
///
/// Shares round down, so they add up to at most the cost attributed to the
/// settlement. As with `settlements.gas_used`, a transaction with two
/// settlements attributes its full cost twice, once per settlement. We do not
/// expect this to happen.
///
/// The `orders` test only holds once the indexer has written an on-chain
/// order's row, hence attribution from `run_optional_maintenance`: attributing
/// earlier would permanently class such an order as liquidity-only.
#[instrument(skip_all)]
pub async fn attribute_gas_cost(
    ex: &mut PgTransaction<'_>,
    settlement: EventIndex,
    gas_used: BigDecimal,
    effective_gas_price: BigDecimal,
    surplus_capturing_jit_order_owners: &[Address],
) -> Result<(), sqlx::Error> {
    // The divisor is never 0: only rows that are in `gas_paying` divide by it.
    // Bytes 33 to 52 of an order uid are the owner, see the
    // `trades_order_uid_owner` index.
    const QUERY: &str = const_format::concatcp!(
        SETTLED_TRADES_CTE,
        ", gas_paying AS (
            SELECT block_number, log_index
            FROM settled s
            WHERE EXISTS (SELECT 1 FROM orders o WHERE o.uid = s.order_uid)
            OR    substring(s.order_uid, 33, 20) = ANY($5)
        )
        UPDATE trades t SET gas_cost =
            CASE
                WHEN p.log_index IS NULL THEN 0
                ELSE FLOOR($3 * $4 / (SELECT COUNT(*) FROM gas_paying))
            END
            FROM settled s
            LEFT JOIN gas_paying p
                ON  p.block_number = s.block_number
                AND p.log_index = s.log_index
            WHERE t.block_number = s.block_number
            AND   t.log_index = s.log_index"
    );
    sqlx::query(QUERY)
        .bind(settlement.block_number)
        .bind(settlement.log_index)
        .bind(gas_used)
        .bind(effective_gas_price)
        .bind(surplus_capturing_jit_order_owners)
        .execute(ex.deref_mut())
        .await
        .map(|_| ())
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

    async fn gas_costs(ex: &mut PgConnection) -> Vec<(i64, Option<BigDecimal>)> {
        sqlx::query_as("SELECT log_index, gas_cost FROM trades ORDER BY log_index")
            .fetch_all(ex)
            .await
            .unwrap()
    }

    /// A settlement's gas cost is split between the trades it settled, and a
    /// second settlement in the same block only takes its own trades.
    #[tokio::test]
    #[ignore]
    async fn postgres_attribute_gas_cost() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event = |log_index| EventIndex {
            block_number: 0,
            log_index,
        };

        // Two settlements in one block: the first settles the trades at log 0
        // and 1, the second those at log 3, 4 and 5.
        for log_index in [0, 1, 3, 4, 5] {
            let uid = ByteArray([u8::try_from(log_index).unwrap(); 56]);
            add_order_and_trade(
                &mut db,
                Default::default(),
                uid,
                event(log_index),
                None,
                None,
            )
            .await;
        }
        let first = event(2);
        add_settlement(&mut db, first, Default::default(), ByteArray([1; 32]), 1).await;
        let second = event(6);
        add_settlement(&mut db, second, Default::default(), ByteArray([2; 32]), 2).await;

        // Nothing is attributed until the settlement is observed.
        assert!(
            gas_costs(&mut db)
                .await
                .iter()
                .all(|(_, cost)| cost.is_none())
        );

        attribute_gas_cost(&mut db, first, 100.into(), 10.into(), &[])
            .await
            .unwrap();

        // 1000 wei split between the first settlement's 2 trades. The second
        // settlement's trades are untouched.
        assert_eq!(
            gas_costs(&mut db).await,
            vec![
                (0, Some(500.into())),
                (1, Some(500.into())),
                (3, None),
                (4, None),
                (5, None),
            ]
        );

        // 700 wei over 3 trades does not divide evenly. The share rounds down,
        // so the shares never add up to more than the transaction paid.
        attribute_gas_cost(&mut db, second, 70.into(), 10.into(), &[])
            .await
            .unwrap();
        assert_eq!(
            gas_costs(&mut db).await,
            vec![
                (0, Some(500.into())),
                (1, Some(500.into())),
                (3, Some(233.into())),
                (4, Some(233.into())),
                (5, Some(233.into())),
            ]
        );
    }

    /// JIT orders only provide liquidity for the user orders, so they take no
    /// share of the gas cost, unless the auction lets their owner capture
    /// surplus.
    #[tokio::test]
    #[ignore]
    async fn postgres_attribute_gas_cost_of_jit_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event = |log_index| EventIndex {
            block_number: 0,
            log_index,
        };
        let uid = |owner: Address| {
            let mut uid = [0u8; 56];
            uid[32..52].copy_from_slice(&owner.0);
            ByteArray(uid)
        };
        let user = ByteArray([1; 20]);
        let market_maker = ByteArray([2; 20]);
        let liquidity_provider = ByteArray([3; 20]);

        // A user order, a JIT order of an owner the auction lets capture
        // surplus and a plain liquidity JIT order.
        add_order_and_trade(&mut db, user, uid(user), event(0), None, None).await;
        add_trade(
            &mut db,
            market_maker,
            uid(market_maker),
            event(1),
            None,
            None,
        )
        .await;
        add_trade(
            &mut db,
            liquidity_provider,
            uid(liquidity_provider),
            event(2),
            None,
            None,
        )
        .await;
        let settlement = event(3);
        add_settlement(
            &mut db,
            settlement,
            Default::default(),
            ByteArray([1; 32]),
            1,
        )
        .await;

        attribute_gas_cost(&mut db, settlement, 100.into(), 10.into(), &[market_maker])
            .await
            .unwrap();

        // 1000 wei split between the user order and the surplus capturing JIT
        // order. The liquidity JIT order paid nothing.
        assert_eq!(
            gas_costs(&mut db).await,
            vec![
                (0, Some(500.into())),
                (1, Some(500.into())),
                (2, Some(0.into())),
            ]
        );
    }

    /// A settlement that settled no trades must not fail on a zero divisor.
    #[tokio::test]
    #[ignore]
    async fn postgres_attribute_gas_cost_without_trades() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let settlement = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        add_settlement(
            &mut db,
            settlement,
            Default::default(),
            Default::default(),
            1,
        )
        .await;

        attribute_gas_cost(&mut db, settlement, 100.into(), 10.into(), &[])
            .await
            .unwrap();
        assert!(gas_costs(&mut db).await.is_empty());
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

        // make sure that for a settlement_a in the same block, only trade_a is
        // returned
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

        // make sure that for a settlement_b in the same block, only trade_b is
        // returned
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

    /// A trade whose settlement was never attributed reports no cost at all.
    #[tokio::test]
    #[ignore]
    async fn postgres_trades_report_attributed_gas_cost() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let users_and_orders = generate_owners_and_order_ids(&[2]).await;
        let (owner, orders) = &users_and_orders[0];
        let event = |log_index| EventIndex {
            block_number: 0,
            log_index,
        };

        // Two settlements in one block: the first settles both orders, the
        // second only order[0]'s next fill.
        add_order_and_trade(&mut db, *owner, orders[0], event(0), None, None).await;
        add_order_and_trade(&mut db, *owner, orders[1], event(1), None, None).await;
        let first = event(2);
        add_settlement(&mut db, first, Default::default(), ByteArray([1; 32]), 1).await;
        add_trade(&mut db, *owner, orders[0], event(3), None, None).await;
        let second = event(4);
        add_settlement(&mut db, second, Default::default(), ByteArray([2; 32]), 2).await;

        // Only the first settlement is attributed.
        attribute_gas_cost(&mut db, first, 100.into(), 10.into(), &[])
            .await
            .unwrap();

        let mut rows = trades(&mut db, Some(owner), None, 0, 1000)
            .into_inner()
            .await
            .unwrap();
        rows.sort_by_key(|row| row.log_index);
        assert_eq!(
            rows.iter()
                .map(|row| (row.order_uid, row.gas_cost.clone(), row.tx_hash))
                .collect::<Vec<_>>(),
            vec![
                // 1000 wei split between the first settlement's 2 trades.
                (orders[0], Some(500.into()), Some(ByteArray([1; 32]))),
                (orders[1], Some(500.into()), Some(ByteArray([1; 32]))),
                (orders[0], None, Some(ByteArray([2; 32]))),
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
