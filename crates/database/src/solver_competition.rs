use {
    crate::{
        auction::AuctionId,
        orders::OrderKind,
        Address,
        OrderUid,
        PgTransaction,
        TransactionHash,
    },
    bigdecimal::BigDecimal,
    sqlx::{types::JsonValue, PgConnection, QueryBuilder},
    std::ops::DerefMut,
};

pub async fn save_solver_competition(
    ex: &mut PgConnection,
    id: AuctionId,
    data: &JsonValue,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO solver_competitions (id, json)
VALUES ($1, $2)
    ;"#;
    sqlx::query(QUERY).bind(id).bind(data).execute(ex).await?;
    Ok(())
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct LoadCompetition {
    pub json: JsonValue,
    pub id: AuctionId,
    // Multiple settlements can be associated with a single competition.
    pub tx_hashes: Vec<TransactionHash>,
}

pub async fn load_by_id(
    ex: &mut PgConnection,
    id: AuctionId,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, COALESCE(ARRAY_AGG(s.tx_hash) FILTER (WHERE so.block_number IS NOT NULL), '{}') AS tx_hashes
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
-- exclude settlements from another environment for which observation is guaranteed to not exist
LEFT OUTER JOIN settlement_observations so 
    ON s.block_number = so.block_number 
    AND s.log_index = so.log_index
WHERE sc.id = $1
GROUP BY sc.id
    ;"#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

pub async fn load_latest_competition(
    ex: &mut PgConnection,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, COALESCE(ARRAY_AGG(s.tx_hash) FILTER (WHERE so.block_number IS NOT NULL), '{}') AS tx_hashes
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
-- exclude settlements from another environment for which observation is guaranteed to not exist
LEFT OUTER JOIN settlement_observations so 
    ON s.block_number = so.block_number 
    AND s.log_index = so.log_index
GROUP BY sc.id
ORDER BY sc.id DESC
LIMIT 1
    ;"#;
    sqlx::query_as(QUERY).fetch_optional(ex).await
}

pub async fn load_by_tx_hash(
    ex: &mut PgConnection,
    tx_hash: &TransactionHash,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
WITH competition AS (
    SELECT sc.id
    FROM solver_competitions sc
    JOIN settlements s ON sc.id = s.auction_id
    JOIN settlement_observations so 
        ON s.block_number = so.block_number 
        AND s.log_index = so.log_index
    WHERE s.tx_hash = $1
)
SELECT sc.json, sc.id, COALESCE(ARRAY_AGG(s.tx_hash) FILTER (WHERE so.block_number IS NOT NULL), '{}') AS tx_hashes
FROM solver_competitions sc
JOIN settlements s ON sc.id = s.auction_id
JOIN settlement_observations so 
    ON s.block_number = so.block_number 
    AND s.log_index = so.log_index
WHERE sc.id = (SELECT id FROM competition)
GROUP BY sc.id
    ;"#;
    sqlx::query_as(QUERY).bind(tx_hash).fetch_optional(ex).await
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Solution {
    // Unique Id generated by the autopilot to uniquely identify the solution within Auction
    pub uid: i64,
    // Id as reported by the solver (solvers are unaware of how other solvers are numbering their
    // solutions)
    pub id: BigDecimal,
    pub solver: Address,
    pub is_winner: bool,
    pub score: BigDecimal,
    pub orders: Vec<Order>,
    // UCP prices
    pub price_tokens: Vec<Address>,
    pub price_values: Vec<BigDecimal>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Order {
    pub uid: OrderUid,
    pub sell_token: Address,
    pub buy_token: Address,
    pub limit_sell: BigDecimal,
    pub limit_buy: BigDecimal,
    pub executed_sell: BigDecimal,
    pub executed_buy: BigDecimal,
    pub side: OrderKind,
}

pub async fn save(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    solutions: &[Solution],
) -> Result<(), sqlx::Error> {
    if solutions.is_empty() {
        return Ok(());
    }

    save_solutions(ex, auction_id, solutions).await?;
    save_trade_executions(ex, auction_id, solutions).await?;
    save_jit_orders(ex, auction_id, solutions).await?;

    Ok(())
}

async fn save_solutions(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    solutions: &[Solution],
) -> Result<(), sqlx::Error> {
    let mut builder = QueryBuilder::new(
        r#"INSERT INTO proposed_solutions 
        (auction_id, uid, id, solver, is_winner, score, price_tokens, price_values)"#,
    );

    builder.push_values(solutions.iter(), |mut b, solution| {
        b.push_bind(auction_id)
            .push_bind(solution.uid)
            .push_bind(&solution.id)
            .push_bind(solution.solver)
            .push_bind(solution.is_winner)
            .push_bind(&solution.score)
            .push_bind(&solution.price_tokens)
            .push_bind(&solution.price_values);
    });

    builder.push(" ON CONFLICT (auction_id, uid) DO NOTHING;");
    builder.build().execute(ex.deref_mut()).await?;
    Ok(())
}

async fn save_trade_executions(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    solutions: &[Solution],
) -> Result<(), sqlx::Error> {
    let mut builder = QueryBuilder::new(
        r#"INSERT INTO proposed_trade_executions 
        (auction_id, solution_uid, order_uid, executed_sell, executed_buy)"#,
    );

    builder.push_values(
        solutions.iter().flat_map(|solution| {
            solution
                .orders
                .iter()
                .map(move |order| (solution.uid, order))
        }),
        |mut b, (solution_uid, order)| {
            b.push_bind(auction_id)
                .push_bind(solution_uid)
                .push_bind(order.uid)
                .push_bind(order.executed_sell.clone())
                .push_bind(order.executed_buy.clone());
        },
    );

    builder.push(" ON CONFLICT (auction_id, solution_uid, order_uid) DO NOTHING;");
    builder.build().execute(ex.deref_mut()).await?;
    Ok(())
}

async fn save_jit_orders(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    solutions: &[Solution],
) -> Result<(), sqlx::Error> {
    for solution in solutions {
        for order in &solution.orders {
            // Order data is saved to `proposed_jit_orders` table only if the order is not
            // already in the `orders` table.
            const QUERY_JIT: &str = r#"
                INSERT INTO proposed_jit_orders 
                (auction_id, solution_uid, order_uid, sell_token, buy_token, limit_sell, limit_buy, side)
                SELECT $1, $2, $3, $4, $5, $6, $7, $8
                    WHERE NOT EXISTS (SELECT 1 FROM orders WHERE uid = $3)
                ON CONFLICT (auction_id, solution_uid, order_uid) DO NOTHING
            "#;

            sqlx::query(QUERY_JIT)
                .bind(auction_id)
                .bind(solution.uid)
                .bind(order.uid)
                .bind(order.sell_token)
                .bind(order.buy_token)
                .bind(order.limit_sell.clone())
                .bind(order.limit_buy.clone())
                .bind(order.side)
                .execute(ex.deref_mut())
                .await?;
        }
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<Solution>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT 
            ps.uid, ps.id, ps.solver, ps.is_winner, ps.score, ps.price_tokens, ps.price_values,
            pse.order_uid, pse.executed_sell, pse.executed_buy,
            COALESCE(pjo.sell_token, o.sell_token) AS sell_token,
            COALESCE(pjo.buy_token, o.buy_token) AS buy_token,
            COALESCE(pjo.limit_sell, o.sell_amount) AS limit_sell,
            COALESCE(pjo.limit_buy, o.buy_amount) AS limit_buy,
            COALESCE(pjo.side, o.kind) AS side
        FROM proposed_solutions ps
        JOIN proposed_trade_executions pse
            ON ps.auction_id = pse.auction_id AND ps.uid = pse.solution_uid
        LEFT JOIN proposed_jit_orders pjo
            ON pse.auction_id = pjo.auction_id AND pse.solution_uid = pjo.solution_uid AND pse.order_uid = pjo.order_uid
        LEFT JOIN orders o
            ON pse.order_uid = o.uid
        WHERE ps.auction_id = $1
    "#;

    #[derive(sqlx::FromRow)]
    struct Row {
        uid: i64,
        id: BigDecimal,
        solver: Address,
        is_winner: bool,
        score: BigDecimal,
        price_tokens: Vec<Address>,
        price_values: Vec<BigDecimal>,
        order_uid: OrderUid,
        executed_sell: BigDecimal,
        executed_buy: BigDecimal,
        sell_token: Address,
        buy_token: Address,
        limit_sell: BigDecimal,
        limit_buy: BigDecimal,
        side: OrderKind,
    }

    let rows: Vec<Row> = sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await?;

    let mut solutions_map = std::collections::HashMap::new();

    for row in rows {
        let order = Order {
            uid: row.order_uid,
            sell_token: row.sell_token,
            buy_token: row.buy_token,
            limit_sell: row.limit_sell,
            limit_buy: row.limit_buy,
            executed_sell: row.executed_sell,
            executed_buy: row.executed_buy,
            side: row.side,
        };

        solutions_map
            .entry(row.uid)
            .or_insert_with(|| Solution {
                uid: row.uid,
                id: row.id,
                solver: row.solver,
                is_winner: row.is_winner,
                score: row.score,
                orders: Vec::new(),
                price_tokens: row.price_tokens,
                price_values: row.price_values,
            })
            .orders
            .push(order);
    }

    // Order by uid to return the solutions in the same order as they were inserted.
    let mut solutions = solutions_map.into_values().collect::<Vec<_>>();
    solutions.sort_by_key(|solution| solution.uid);
    Ok(solutions)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            byte_array::ByteArray,
            events::{EventIndex, Settlement},
        },
        sqlx::{Connection, Row},
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Bool(true);
        save_solver_competition(&mut db, 0, &value).await.unwrap();

        // load by id works
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hashes.is_empty());
        // load as latest works
        let value_ = load_latest_competition(&mut db).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hashes.is_empty());
        // load by tx doesn't work, as there is no settlement yet
        assert!(load_by_tx_hash(&mut db, &ByteArray([0u8; 32]))
            .await
            .unwrap()
            .is_none());

        // non-existent auction returns none
        assert!(load_by_id(&mut db, 1).await.unwrap().is_none());

        // insert three settlement events for the same auction id, with one of them not
        // having observation (in practice usually meaning it's from different
        // environment)
        crate::events::insert_settlement(
            &mut db,
            &EventIndex {
                block_number: 0,
                log_index: 0,
            },
            &Settlement {
                solver: Default::default(),
                transaction_hash: ByteArray([0u8; 32]),
            },
        )
        .await
        .unwrap();
        crate::settlement_observations::upsert(
            &mut db,
            crate::settlement_observations::Observation {
                block_number: 0,
                log_index: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::events::insert_settlement(
            &mut db,
            &EventIndex {
                block_number: 0,
                log_index: 1,
            },
            &Settlement {
                solver: Default::default(),
                transaction_hash: ByteArray([1u8; 32]),
            },
        )
        .await
        .unwrap();
        crate::settlement_observations::upsert(
            &mut db,
            crate::settlement_observations::Observation {
                block_number: 0,
                log_index: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::events::insert_settlement(
            &mut db,
            &EventIndex {
                block_number: 0,
                log_index: 2,
            },
            &Settlement {
                solver: Default::default(),
                transaction_hash: ByteArray([2u8; 32]),
            },
        )
        .await
        .unwrap();
        crate::settlements::update_settlement_auction(&mut db, 0, 0, 0)
            .await
            .unwrap();
        crate::settlements::update_settlement_auction(&mut db, 0, 1, 0)
            .await
            .unwrap();
        crate::settlements::update_settlement_auction(&mut db, 0, 2, 0)
            .await
            .unwrap();

        // load by id works, and finds two hashes
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert!(value_.tx_hashes.len() == 2);

        // load as latest works, and finds two hashes
        let value_ = load_latest_competition(&mut db).await.unwrap().unwrap();
        assert!(value_.tx_hashes.len() == 2);

        // load by tx works, and finds two hashes, no matter which tx hash is used
        let value_ = load_by_tx_hash(&mut db, &ByteArray([0u8; 32]))
            .await
            .unwrap()
            .unwrap();
        assert!(value_.tx_hashes.len() == 2);
        let value_ = load_by_tx_hash(&mut db, &ByteArray([1u8; 32]))
            .await
            .unwrap()
            .unwrap();
        assert!(value_.tx_hashes.len() == 2);
        // this one should not find any hashes since it's from another environment
        let value_ = load_by_tx_hash(&mut db, &ByteArray([2u8; 32]))
            .await
            .unwrap();
        assert!(value_.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_solutions_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // insert an order to "orders" table to prevent one of the orders from being
        // inserted into the proposed_jit_orders table
        let user_order_uid = ByteArray([5u8; 56]);
        let order = crate::orders::Order {
            uid: user_order_uid,
            ..Default::default()
        };
        crate::orders::insert_order(&mut db, &order).await.unwrap();

        let solutions = vec![
            Solution {
                uid: 0,
                id: 0.into(),
                solver: ByteArray([1u8; 20]), // from solver 1
                orders: vec![Default::default()],
                ..Default::default()
            },
            Solution {
                uid: 1,
                id: 0.into(),
                solver: ByteArray([2u8; 20]), // from solver 2
                orders: vec![Default::default()],
                ..Default::default()
            },
            Solution {
                uid: 2,
                id: 1.into(),
                solver: ByteArray([2u8; 20]), // from solver 2
                orders: vec![
                    Order {
                        uid: ByteArray([1u8; 56]),
                        ..Default::default()
                    },
                    // this one should not be inserted into the proposed_jit_orders as it already
                    // exists in the orders table
                    Order {
                        uid: user_order_uid,
                        ..Default::default()
                    },
                    Order {
                        uid: ByteArray([6u8; 56]),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        ];

        save(&mut db, 0, &solutions).await.unwrap();
        let fetched_solutions = fetch(&mut db, 0).await.unwrap();

        // first two solutions should be identical
        assert_eq!(solutions[0..2], fetched_solutions[0..2]);

        let proposed_jit_orders =
            sqlx::query("SELECT order_uid FROM proposed_jit_orders ORDER BY order_uid")
                .fetch_all(db.deref_mut())
                .await
                .unwrap()
                .into_iter()
                .map(|row| row.get::<OrderUid, _>(0))
                .collect::<Vec<_>>();
        // proposed_jit_orders should contain only the orders that were not already in
        // the "orders"
        assert_eq!(
            proposed_jit_orders,
            vec![
                solutions[0].orders[0].uid,
                solutions[1].orders[0].uid,
                solutions[2].orders[0].uid,
                solutions[2].orders[2].uid,
            ]
        );

        // but when solution 3 is fetched, it should have the same orders that were
        // inserted (2 fetched from "proposed_jit_orders" and 1 from "orders" table)
        assert!(fetched_solutions[2].orders.len() == 3);
    }
}
