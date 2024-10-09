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
    sqlx::{types::JsonValue, PgConnection},
    std::ops::DerefMut,
};

pub async fn save(
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
    pub tx_hash: Option<TransactionHash>,
}

pub async fn load_by_id(
    ex: &mut PgConnection,
    id: AuctionId,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, s.tx_hash
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
WHERE sc.id = $1
    ;"#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

pub async fn load_latest_competition(
    ex: &mut PgConnection,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, s.tx_hash
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
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
SELECT sc.json, sc.id, s.tx_hash
FROM solver_competitions sc
JOIN settlements s ON sc.id = s.auction_id
WHERE s.tx_hash = $1
    ;"#;
    sqlx::query_as(QUERY).bind(tx_hash).fetch_optional(ex).await
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Solution {
    // Unique Id generated by the autopilot to uniquely identify the solution within Auction
    pub uid: i64,
    // Id as reported by the solver (solvers are unaware of how other solvers are numbering their
    // solutions)
    pub id: i64,
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

pub async fn save_solutions(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    solutions: &[Solution],
) -> Result<(), sqlx::Error> {
    // todo merge into three queries
    for solution in solutions {
        const QUERY: &str = r#"
            INSERT INTO proposed_solutions (auction_id, uid, id, solver, is_winner, score, price_tokens, price_values)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (auction_id, uid) DO NOTHING
        "#;
        sqlx::query(QUERY)
            .bind(auction_id)
            .bind(solution.uid)
            .bind(solution.id)
            .bind(solution.solver)
            .bind(solution.is_winner)
            .bind(&solution.score)
            .bind(&solution.price_tokens)
            .bind(&solution.price_values)
            .execute(ex.deref_mut())
            .await?;

        for order in &solution.orders {
            const QUERY: &str = r#"
                INSERT INTO proposed_solution_executions (
                    auction_id, solution_uid, order_uid, executed_sell, executed_buy
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (auction_id, solution_uid, order_uid) DO NOTHING
            "#;

            sqlx::query(QUERY)
                .bind(auction_id)
                .bind(solution.uid)
                .bind(order.uid)
                .bind(order.executed_sell.clone())
                .bind(order.executed_buy.clone())
                .execute(ex.deref_mut())
                .await?;

            // Order data is saved to `proposed_jit_orders` table only if the order is not
            // already in the `orders` table.
            const QUERY_JIT: &str = r#"
                INSERT INTO proposed_jit_orders (
                    auction_id, solution_uid, order_uid, sell_token, buy_token, limit_sell, limit_buy, side
                )
                SELECT $1, $2, $3, $4, $5, $6, $7, $8
                WHERE NOT EXISTS (
                    SELECT 1 FROM orders WHERE uid = $3
                )
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
pub async fn fetch_solutions(
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
        LEFT JOIN proposed_solution_executions pse
            ON ps.auction_id = pse.auction_id AND ps.uid = pse.solution_uid
        LEFT JOIN proposed_jit_orders pjo
            ON pse.auction_id = pjo.auction_id AND pse.solution_uid = pjo.solution_uid AND pse.order_uid = pjo.order_uid
        LEFT JOIN orders o
            ON pse.order_uid = o.uid
        WHERE ps.auction_id = $1
    "#;

    let rows: Vec<(
        i64,
        i64,
        Address,
        bool,
        BigDecimal,
        Vec<Address>,
        Vec<BigDecimal>,
        OrderUid,
        BigDecimal,
        BigDecimal,
        Address,
        Address,
        BigDecimal,
        BigDecimal,
        OrderKind,
    )> = sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await?;

    let mut solutions_map = std::collections::HashMap::new();

    for row in rows {
        let (
            uid,
            id,
            solver,
            is_winner,
            score,
            price_tokens,
            price_values,
            order_uid,
            executed_sell,
            executed_buy,
            sell_token,
            buy_token,
            limit_sell,
            limit_buy,
            side,
        ) = row;

        let order = Order {
            uid: order_uid,
            sell_token,
            buy_token,
            limit_sell,
            limit_buy,
            executed_sell,
            executed_buy,
            side,
        };

        solutions_map
            .entry(uid)
            .or_insert_with(|| Solution {
                uid,
                id,
                solver,
                is_winner,
                score,
                orders: Vec::new(),
                price_tokens,
                price_values,
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
            events::{Event, EventIndex, Settlement},
        },
        sqlx::Connection,
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Bool(true);
        save(&mut db, 0, &value).await.unwrap();
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hash.is_none());

        assert!(load_by_id(&mut db, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_by_hash() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let id: i64 = 5;
        let value = JsonValue::Bool(true);
        let hash = ByteArray([1u8; 32]);
        save(&mut db, id, &value).await.unwrap();

        let value_by_id = load_by_id(&mut db, id).await.unwrap().unwrap();
        assert_eq!(value, value_by_id.json);
        // no hash because hash columns isn't used to find it
        assert!(value_by_id.tx_hash.is_none());

        // Fails because the tx_hash stored directly in the solver_competitions table is
        // no longer used to look the competition up.
        assert!(load_by_tx_hash(&mut db, &hash).await.unwrap().is_none());

        // Now insert the proper settlement event and account-nonce.

        let index = EventIndex::default();
        let event = Event::Settlement(Settlement {
            solver: Default::default(),
            transaction_hash: hash,
        });
        crate::events::append(&mut db, &[(index, event)])
            .await
            .unwrap();

        crate::settlements::update_settlement_auction(
            &mut db,
            index.block_number,
            index.log_index,
            id,
        )
        .await
        .unwrap();

        // Now succeeds.
        let value_by_hash = load_by_tx_hash(&mut db, &hash).await.unwrap().unwrap();
        assert_eq!(value, value_by_hash.json);
        assert_eq!(id, value_by_hash.id);

        // By id also sees the hash now.
        let value_by_id = load_by_id(&mut db, id).await.unwrap().unwrap();
        assert_eq!(hash, value_by_id.tx_hash.unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_solutions_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let solutions = vec![
            Solution {
                uid: 0,
                id: 0,
                solver: ByteArray([1u8; 20]), // from solver 1
                orders: vec![Default::default()],
                ..Default::default()
            },
            Solution {
                uid: 1,
                id: 0,
                solver: ByteArray([2u8; 20]), // from solver 2
                orders: vec![Default::default()],
                ..Default::default()
            },
            Solution {
                uid: 2,
                id: 1,
                solver: ByteArray([2u8; 20]), // from solver 2
                orders: vec![Default::default()],
                ..Default::default()
            },
        ];

        save_solutions(&mut db, 0, &solutions).await.unwrap();
        let solutions_ = fetch_solutions(&mut db, 0).await.unwrap();
        assert_eq!(solutions, solutions_);
    }
}
