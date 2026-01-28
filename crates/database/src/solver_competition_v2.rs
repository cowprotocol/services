//! This file contains all functions related to reading or updating
//! data about the competition during an auction in the new tables
//! dedicated for that.
//! See `solver_competition.rs` for the legacy version of this which
//! simply stored JSON blobs in the DB.

use {
    crate::{
        Address,
        OrderUid,
        PgTransaction,
        TransactionHash,
        auction::AuctionId,
        orders::OrderKind,
    },
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
    std::ops::DerefMut,
    tracing::instrument,
};

#[derive(sqlx::FromRow)]
pub struct Settlement {
    pub solution_uid: i64,
    pub tx_hash: TransactionHash,
}

#[derive(sqlx::FromRow)]
pub struct Auction {
    pub order_uids: Vec<OrderUid>,
    pub price_tokens: Vec<Address>,
    pub price_values: Vec<BigDecimal>,
    pub block: i64,
    pub id: i64,
    pub deadline: i64,
}

#[derive(sqlx::FromRow)]
pub struct ProposedSolution {
    pub solver: Address,
    pub uid: i64,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub score: BigDecimal,
    pub price_tokens: Vec<Address>,
    pub price_values: Vec<BigDecimal>,
    pub ranking: i64,
}

#[derive(sqlx::FromRow)]
pub struct ProposedTrade {
    pub solution_uid: i64,
    pub order_uid: OrderUid,
    pub executed_sell: BigDecimal,
    pub executed_buy: BigDecimal,
    pub sell_token: Address,
    pub buy_token: Address,
}

#[derive(sqlx::FromRow)]
pub struct ReferenceScore {
    pub solver: Address,
    pub reference_score: BigDecimal,
}

pub struct SolverCompetition {
    pub settlements: Vec<Settlement>,
    pub auction: Auction,
    pub solutions: Vec<ProposedSolution>,
    pub trades: Vec<ProposedTrade>,
    pub reference_scores: Vec<ReferenceScore>,
}

#[instrument(skip_all)]
pub async fn load_by_tx_hash(
    mut ex: &mut PgConnection,
    tx_hash: TransactionHash,
) -> Result<Option<SolverCompetition>, sqlx::Error> {
    const FETCH_AUCTION_ID: &str = r#"
        SELECT s.auction_id
        FROM settlements s
        WHERE s.tx_hash = $1 AND s.auction_id IS NOT NULL AND s.solution_uid IS NOT NULL;
    "#;
    let auction_id = sqlx::query_scalar(FETCH_AUCTION_ID)
        .bind(tx_hash)
        .fetch_optional(ex.deref_mut())
        .await?;
    let Some(auction_id) = auction_id else {
        return Ok(None);
    };
    load_by_id(ex.deref_mut(), auction_id).await
}

#[instrument(skip_all)]
pub async fn load_latest(
    mut ex: &mut PgConnection,
) -> Result<Option<SolverCompetition>, sqlx::Error> {
    const FETCH_AUCTION_ID: &str = r#"
        SELECT id
        FROM competition_auctions
        ORDER BY id DESC
        LIMIT 1;
    "#;
    let auction_id: Option<i64> = sqlx::query_scalar(FETCH_AUCTION_ID)
        .fetch_optional(ex.deref_mut())
        .await?;
    let Some(auction_id) = auction_id else {
        return Ok(None);
    };
    load_by_id(ex.deref_mut(), auction_id).await
}

#[instrument(skip_all)]
pub async fn load_by_id(
    mut ex: &mut PgConnection,
    id: AuctionId,
) -> Result<Option<SolverCompetition>, sqlx::Error> {
    const FETCH_AUCTION: &str = r#"
        SELECT id, order_uids, price_tokens, price_values, block, deadline
        FROM competition_auctions
        WHERE id = $1;
    "#;
    let auction: Option<Auction> = sqlx::query_as(FETCH_AUCTION)
        .bind(id)
        .fetch_optional(ex.deref_mut())
        .await?;
    let Some(auction) = auction else {
        return Ok(None);
    };

    const FETCH_SETTLEMENTS: &str = r#"
        SELECT s.solution_uid, s.tx_hash
        FROM settlements s
        WHERE s.auction_id = $1 AND s.solution_uid IS NOT NULL AND s.solver IS NOT NULL;
    "#;
    let settlements: Vec<Settlement> = sqlx::query_as(FETCH_SETTLEMENTS)
        .bind(id)
        .fetch_all(ex.deref_mut())
        .await?;

    // we set `ranking to uid + 1` because uids get assigned from best to worst
    // solution starting at 0
    const FETCH_SOLUTIONS: &str = r#"
        SELECT uid, uid + 1 as ranking, solver, is_winner, filtered_out, score, price_tokens, price_values
        FROM proposed_solutions
        WHERE auction_id = $1;
    "#;
    let solutions: Vec<ProposedSolution> = sqlx::query_as(FETCH_SOLUTIONS)
        .bind(id)
        .fetch_all(ex.deref_mut())
        .await?;

    const FETCH_TRADES: &str = r#"
        SELECT pte.solution_uid, pte.order_uid, executed_sell, executed_buy, 
            COALESCE(o.sell_token, pjo.sell_token) AS sell_token,
            COALESCE(o.buy_token, pjo.buy_token) AS buy_token
        FROM proposed_trade_executions AS pte
        LEFT JOIN orders o ON
            pte.order_uid = o.uid
        LEFT JOIN proposed_jit_orders pjo ON
            pte.order_uid = pjo.order_uid
            AND pte.solution_uid = pjo.solution_uid
            AND pte.auction_id = pjo.auction_id
        WHERE pte.auction_id = $1
            AND COALESCE(o.sell_token, pjo.sell_token) IS NOT NULL
            AND COALESCE(o.buy_token, pjo.buy_token) IS NOT NULL;
    "#;
    let trades: Vec<ProposedTrade> = sqlx::query_as(FETCH_TRADES)
        .bind(id)
        .fetch_all(ex.deref_mut())
        .await?;

    const FETCH_REFERENCE_SCORES: &str = r#"
        SELECT solver, reference_score
        FROM reference_scores
        WHERE auction_id = $1;
    "#;
    let reference_scores: Vec<ReferenceScore> = sqlx::query_as(FETCH_REFERENCE_SCORES)
        .bind(id)
        .fetch_all(ex.deref_mut())
        .await?;

    Ok(Some(SolverCompetition {
        auction,
        settlements,
        solutions,
        trades,
        reference_scores,
    }))
}

/// Participant of a solver competition for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AuctionParticipant {
    pub auction_id: AuctionId,
    pub participant: Address,
}

pub async fn fetch_auction_participants(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<AuctionParticipant>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT DISTINCT ps.solver AS participant, ps.auction_id
        FROM proposed_solutions ps
        WHERE ps.auction_id = $1
    "#;

    sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await
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
    pub filtered_out: bool,
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

#[instrument(skip_all)]
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

#[instrument(skip_all)]
async fn save_solutions(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    solutions: &[Solution],
) -> Result<(), sqlx::Error> {
    let mut builder = QueryBuilder::new(
        r#"INSERT INTO proposed_solutions 
        (auction_id, uid, id, solver, is_winner, filtered_out, score, price_tokens, price_values)"#,
    );

    builder.push_values(solutions.iter(), |mut b, solution| {
        b.push_bind(auction_id)
            .push_bind(solution.uid)
            .push_bind(&solution.id)
            .push_bind(solution.solver)
            .push_bind(solution.is_winner)
            .push_bind(solution.filtered_out)
            .push_bind(&solution.score)
            .push_bind(&solution.price_tokens)
            .push_bind(&solution.price_values);
    });

    builder.push(" ON CONFLICT (auction_id, uid) DO NOTHING;");
    builder.build().execute(ex.deref_mut()).await?;
    Ok(())
}

#[instrument(skip_all)]
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

#[instrument(skip_all)]
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

#[derive(sqlx::FromRow)]
struct SolutionRow {
    uid: i64,
    id: BigDecimal,
    solver: Address,
    is_winner: bool,
    filtered_out: bool,
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

const BASE_SOLUTIONS_QUERY: &str = r#"
    SELECT
        ps.uid, ps.id, ps.solver, ps.is_winner, ps.filtered_out,
        ps.score, ps.price_tokens, ps.price_values,
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
"#;

#[instrument(skip_all)]
pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<Solution>, sqlx::Error> {
    let query_str = format!("{BASE_SOLUTIONS_QUERY} WHERE ps.auction_id = $1");
    let query = sqlx::query_as::<_, SolutionRow>(&query_str).bind(auction_id);

    map_rows_to_solutions(query.fetch_all(ex).await?)
}

#[instrument(skip_all)]
pub async fn fetch_solver_winning_solutions(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    solver: Address,
) -> Result<Vec<Solution>, sqlx::Error> {
    let query_str = format!(
        r#"{BASE_SOLUTIONS_QUERY} WHERE ps.auction_id = $1 AND ps.solver = $2 AND ps.is_winner = TRUE"#
    );
    let query = sqlx::query_as::<_, SolutionRow>(&query_str)
        .bind(auction_id)
        .bind(solver);

    map_rows_to_solutions(query.fetch_all(ex).await?)
}

#[instrument(skip_all)]
fn map_rows_to_solutions(rows: Vec<SolutionRow>) -> Result<Vec<Solution>, sqlx::Error> {
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
                filtered_out: row.filtered_out,
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

/// Fetches all orders for which we must assume that there are
/// still onchain transactions being mined or submitted.
///
/// Those are all orders (JIT or regular) that belong to winning
/// solutions with a deadline greater than the current block
/// where the execution actually has not been observed onchain yet.
pub async fn fetch_in_flight_orders(
    ex: &mut PgConnection,
    current_block: i64,
) -> Result<Vec<OrderUid>, sqlx::Error> {
    const QUERY: &str = r#"
    SELECT DISTINCT order_uid
    FROM competition_auctions ca
    JOIN proposed_solutions ps ON ps.auction_id = ca.id
    JOIN proposed_trade_executions pte ON pte.auction_id = ca.id AND pte.solution_uid = ps.uid
    WHERE ca.deadline > $1
        AND ps.is_winner = true
        AND NOT EXISTS (
            SELECT 1 FROM settlements s
            WHERE s.auction_id = ca.id AND s.solution_uid = ps.uid
        );
    "#;

    sqlx::query_as(QUERY)
        .bind(current_block)
        .fetch_all(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            auction,
            byte_array::ByteArray,
            events::{self, EventIndex, Settlement},
            orders::insert_order_and_ignore_conflicts,
            reference_scores,
            settlements,
        },
        sqlx::{Connection, Row},
    };

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
                is_winner: true,
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

        let solver_winning_solutions =
            fetch_solver_winning_solutions(&mut db, 0, ByteArray([2u8; 20]))
                .await
                .unwrap();
        // The solver has 2 solutions, but only one of them is winning
        assert_eq!(solver_winning_solutions.len(), 1);
        assert_eq!(solver_winning_solutions[0].uid, 2);

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

    #[tokio::test]
    #[ignore]
    async fn postgres_load_by_tx_hash() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let tx_hash = ByteArray([1u8; 32]);
        let settlement = Settlement {
            solver: ByteArray([1u8; 20]),
            transaction_hash: tx_hash,
        };
        events::insert_settlement(&mut db, &event, &settlement)
            .await
            .unwrap();

        let solver_competition = load_by_tx_hash(&mut db, tx_hash).await.unwrap();
        assert!(solver_competition.is_none());

        let solver_competition = load_by_tx_hash(&mut db, tx_hash).await.unwrap();
        assert!(solver_competition.is_none());

        // update settlements
        settlements::update_settlement_auction(&mut db, 1, 0, 1)
            .await
            .unwrap();
        settlements::update_settlement_solver(&mut db, 1, 0, ByteArray([1u8; 20]), 0)
            .await
            .unwrap();

        // competition_auctions
        let auction = auction::Auction {
            id: 1,
            block: 1,
            deadline: 2,
            order_uids: vec![ByteArray([1u8; 56])],
            price_tokens: vec![ByteArray([1u8; 20])],
            price_values: vec![BigDecimal::from(100)],
            surplus_capturing_jit_order_owners: vec![],
        };
        auction::save(&mut db, auction).await.unwrap();

        let solver_competition = load_by_tx_hash(&mut db, tx_hash).await.unwrap();
        assert!(solver_competition.is_some());
        let solver_competition = solver_competition.unwrap();
        assert_eq!(solver_competition.settlements.len(), 1);
        assert_eq!(
            solver_competition.settlements.first().unwrap().tx_hash,
            tx_hash
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_load_by_id() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let block_number = 1;
        let log_index = 0;
        let auction_id = 1;

        let solver_competition = load_by_id(&mut db, auction_id).await.unwrap();
        assert!(solver_competition.is_none());

        // example order
        let order_uid = ByteArray([1u8; 56]);
        let order_sell_token = ByteArray([1u8; 20]);
        let order_buy_token = ByteArray([2u8; 20]);
        let order_limit_sell = BigDecimal::from(100);
        let order_limit_buy = BigDecimal::from(200);
        let order_executed_sell = BigDecimal::from(50);
        let order_executed_buy = BigDecimal::from(150);
        let order_side = OrderKind::Sell;

        // competition_auctions
        let auction = auction::Auction {
            id: auction_id,
            block: block_number,
            deadline: 2,
            order_uids: vec![order_uid],
            price_tokens: vec![order_sell_token],
            price_values: vec![order_limit_sell.clone()],
            surplus_capturing_jit_order_owners: vec![],
        };
        auction::save(&mut db, auction).await.unwrap();

        let solver_competition = load_by_id(&mut db, auction_id).await.unwrap();
        assert!(solver_competition.is_some());

        // settlements
        let event = EventIndex {
            block_number,
            log_index,
        };
        let tx_hash = ByteArray([1u8; 32]);
        let settlement = Settlement {
            solver: ByteArray([1u8; 20]),
            transaction_hash: tx_hash,
        };
        events::insert_settlement(&mut db, &event, &settlement)
            .await
            .unwrap();

        // Check before the joined table is populated
        let solver_competition = load_by_id(&mut db, auction_id).await.unwrap();
        assert!(solver_competition.is_some());
        let solver_competition = solver_competition.unwrap();
        assert!(solver_competition.settlements.is_empty());
        assert_eq!(solver_competition.auction.id, 1);

        // Check after the joined table is populated
        let solver_competition = load_by_id(&mut db, auction_id).await.unwrap();
        assert!(solver_competition.is_some());
        let solver_competition = solver_competition.unwrap();
        assert!(solver_competition.settlements.is_empty());
        assert_eq!(solver_competition.auction.id, 1);

        // update settlements
        settlements::update_settlement_auction(&mut db, block_number, log_index, auction_id)
            .await
            .unwrap();
        settlements::update_settlement_solver(
            &mut db,
            block_number,
            log_index,
            ByteArray([1u8; 20]),
            0,
        )
        .await
        .unwrap();

        // Check after both tables are linked
        let solver_competition = load_by_id(&mut db, auction_id).await.unwrap();
        assert!(solver_competition.is_some());
        let solver_competition = solver_competition.unwrap();
        assert_eq!(solver_competition.settlements.len(), 1);
        assert_eq!(
            solver_competition.settlements.first().unwrap().tx_hash,
            tx_hash
        );
        assert_eq!(solver_competition.auction.id, 1);

        // proposed_solutions
        let solutions = vec![Solution {
            uid: 0,
            id: 0.into(),
            solver: ByteArray([1u8; 20]),
            is_winner: true,
            filtered_out: false,
            score: BigDecimal::from(100),
            orders: vec![Order {
                uid: order_uid,
                sell_token: order_sell_token,
                buy_token: order_buy_token,
                limit_sell: order_limit_sell.clone(),
                limit_buy: order_limit_buy.clone(),
                executed_sell: order_executed_sell,
                executed_buy: order_executed_buy,
                side: order_side,
            }],
            price_tokens: vec![ByteArray([1u8; 20])],
            price_values: vec![BigDecimal::from(100)],
        }];
        save_solutions(&mut db, auction_id, &solutions)
            .await
            .unwrap();

        // proposed_trade_executions
        save_trade_executions(&mut db, auction_id, &solutions)
            .await
            .unwrap();

        // reference_scores
        let scores = vec![reference_scores::Score {
            auction_id,
            solver: ByteArray([1u8; 20]),
            reference_score: BigDecimal::from(100),
        }];
        reference_scores::insert(&mut db, &scores).await.unwrap();

        // orders
        insert_order_and_ignore_conflicts(
            &mut db,
            &crate::orders::Order {
                uid: order_uid,
                sell_token: order_sell_token,
                buy_token: order_buy_token,
                sell_amount: order_limit_sell.clone(),
                buy_amount: order_limit_buy.clone(),
                kind: order_side,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let solver_competition = load_by_id(&mut db, auction_id).await.unwrap();
        assert!(solver_competition.is_some());
        let solver_competition = solver_competition.unwrap();
        assert_eq!(solver_competition.settlements.len(), 1);
        assert_eq!(
            solver_competition.settlements.first().unwrap().tx_hash,
            tx_hash
        );
        assert_eq!(solver_competition.auction.id, 1);
        assert_eq!(solver_competition.auction.deadline, 2);
        assert_eq!(solver_competition.trades.len(), 1);
        assert_eq!(solver_competition.trades.first().unwrap().solution_uid, 0);
        assert_eq!(solver_competition.reference_scores.len(), 1);
        assert_eq!(solver_competition.solutions.len(), 1);
        assert_eq!(solver_competition.solutions.first().unwrap().uid, 0);

        let auction_participants = fetch_auction_participants(&mut db, auction_id)
            .await
            .unwrap();
        assert_eq!(auction_participants.len(), 1);
        assert_eq!(auction_participants[0].participant, solutions[0].solver);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_fetch_inflight_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_uid = |i| ByteArray([i; 56]);
        let order = |i| Order {
            uid: order_uid(i),
            ..Default::default()
        };
        let solutions = vec![
            Solution {
                uid: 0,
                id: 0.into(),
                orders: vec![order(0)],
                is_winner: true,
                ..Default::default()
            },
            Solution {
                uid: 1,
                id: 0.into(),
                orders: vec![order(1)],
                is_winner: true,
                ..Default::default()
            },
        ];
        crate::auction::save(
            &mut db,
            crate::auction::Auction {
                id: 0,
                block: 0,
                deadline: 5,
                order_uids: Default::default(),
                price_tokens: Default::default(),
                price_values: Default::default(),
                surplus_capturing_jit_order_owners: Default::default(),
            },
        )
        .await
        .unwrap();
        save(&mut db, 0, &solutions).await.unwrap();

        let solutions = vec![
            Solution {
                uid: 2,
                id: 1.into(),
                orders: vec![order(2)],
                is_winner: true,
                ..Default::default()
            },
            Solution {
                uid: 3,
                id: 1.into(),
                orders: vec![order(3)],
                is_winner: true,
                ..Default::default()
            },
        ];
        crate::auction::save(
            &mut db,
            crate::auction::Auction {
                id: 1,
                block: 5,
                deadline: 10,
                order_uids: Default::default(),
                price_tokens: Default::default(),
                price_values: Default::default(),
                surplus_capturing_jit_order_owners: Default::default(),
            },
        )
        .await
        .unwrap();
        save(&mut db, 1, &solutions).await.unwrap();

        // all orders in flight at block 4
        let early_block = fetch_in_flight_orders(&mut db, 4).await.unwrap();
        assert_eq!(early_block.len(), 4);
        assert!(
            [0, 1, 2, 3]
                .into_iter()
                .all(|id| early_block.contains(&order_uid(id)))
        );

        // only orders from the later auction in flight at block 5
        let later_block = fetch_in_flight_orders(&mut db, 5).await.unwrap();
        assert_eq!(later_block.len(), 2);
        assert!(
            [2, 3]
                .into_iter()
                .all(|id| later_block.contains(&order_uid(id)))
        );

        // observe settlement event
        crate::events::insert_settlement(
            &mut db,
            &EventIndex {
                block_number: 5,
                log_index: 0,
            },
            &Default::default(),
        )
        .await
        .unwrap();
        // associate with auction 1
        settlements::update_settlement_auction(&mut db, 5, 0, 1)
            .await
            .unwrap();
        // associate with solution 3
        settlements::update_settlement_solver(&mut db, 5, 0, Default::default(), 3)
            .await
            .unwrap();

        // when an order gets marked as settled we dont consider it inflight anymore
        let later_block_with_settlement = fetch_in_flight_orders(&mut db, 5).await.unwrap();
        assert_eq!(later_block_with_settlement, vec![order_uid(2)]);
    }
}
