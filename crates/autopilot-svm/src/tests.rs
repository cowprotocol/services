//! The BE-184 checkpoint: one full auction cycle through [`AuctionLoop`]
//! against the real database and a mocked driver.

use {
    crate::{
        domain::{arbitrator::SolanaArbitrator, cycle::SolanaCycle},
        infra::{
            competition::DriverCompetition,
            driver::{Driver, dto},
            executor::DriverExecutor,
            observer::LogObserver,
            provider::DbAuctionProvider,
        },
        run_loop::{
            AuctionLoop,
            AuctionProvider,
            CycleTrigger,
            RankingInfo,
            SolverCompetition,
            WinnerSelection,
        },
    },
    async_trait::async_trait,
    axum::{Json, Router, extract::State, routing::post},
    chain_types::solana::{IntentHash, Pubkey, Signature},
    database::byte_array::ByteArray,
    sqlx::PgPool,
    std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration},
    tokio::sync::mpsc,
    url::Url,
};

/// A trigger pinned to one slot: the test drives exactly one cycle.
struct FixedTrigger(u64);

#[async_trait]
impl CycleTrigger<SolanaCycle> for FixedTrigger {
    async fn next_cycle(&mut self) -> u64 {
        self.0
    }

    fn current_tip(&self) -> u64 {
        self.0
    }
}

#[derive(Clone)]
struct MockDriverState {
    solution: dto::Solution,
    settles: mpsc::UnboundedSender<dto::SettleRequest>,
}

async fn handle_solve(State(state): State<MockDriverState>) -> Json<serde_json::Value> {
    Json(
        serde_json::to_value(&dto::SolveResponse {
            solutions: vec![state.solution.clone()],
        })
        .unwrap(),
    )
}

async fn handle_settle(
    State(state): State<MockDriverState>,
    Json(request): Json<dto::SettleRequest>,
) -> Json<serde_json::Value> {
    state.settles.send(request).unwrap();
    Json(
        serde_json::to_value(&dto::SettleResponse {
            tx_signature: Signature([9; 64]),
        })
        .unwrap(),
    )
}

/// Serves `/solve` with one canned solution and records every `/settle`.
async fn spawn_mock_driver(state: MockDriverState) -> SocketAddr {
    let app = Router::new()
        .route("/solve", post(handle_solve))
        .route("/settle", post(handle_settle))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

async fn seed_open_order(pool: &PgPool, uid: [u8; 32], tip: i64) {
    sqlx::query(
        "TRUNCATE solana.trades, solana.settlements, solana.order_pda, solana.orders, \
         solana.indexer_state",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO solana.indexer_state (slot) VALUES ($1)")
        .bind(tip)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, order_pda)
VALUES ($1, $2, $2, $3, $2, $2, 1000, 500, $4, 'sell'::OrderKind, false, $2, now(), $5)
        "#,
    )
    .bind(uid)
    .bind(ByteArray([0xAA; 32]))
    .bind(ByteArray([0xAB; 32]))
    .bind(i64::from(u32::MAX))
    .bind(ByteArray([0xB0; 32]))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO solana.order_pda (order_uid, created_by) VALUES ($1, $2)")
        .bind(uid)
        .bind(ByteArray([0xAA; 32]))
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_mock_cycle_dispatches_the_settlement() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    let uid = [0x11; 32];
    let tip = 500_u64;
    seed_open_order(&pool, uid, i64::try_from(tip).unwrap()).await;

    // Selling 1000 for at least 500 and receiving 600 clears the limit, so
    // the solution scores its 100 surplus and wins.
    let solution = dto::Solution {
        solution_id: 7,
        score: 100,
        solver: Pubkey([0xCC; 32]),
        orders: HashMap::from([(
            IntentHash(uid),
            dto::TradedAmounts {
                executed_sell: 1000,
                executed_buy: 600,
            },
        )]),
    };
    let wrapped_native = Pubkey([0xFE; 32]);
    let (settles, mut settled) = mpsc::unbounded_channel();
    let addr = spawn_mock_driver(MockDriverState { solution, settles }).await;
    let driver = Arc::new(Driver::new(
        "mock".to_string(),
        &Url::parse(&format!("http://{addr}")).unwrap(),
    ));

    // Stage probes: pinpoint the failing phase before driving the loop.
    {
        let provider = DbAuctionProvider::new(pool.clone());
        let auction = provider.cut_auction(&tip).await.expect("auction cut");
        assert_eq!(auction.orders.len(), 1, "open order in the auction");
        let competition = DriverCompetition::new(vec![Arc::clone(&driver)], Duration::from_secs(6));
        let solutions = competition.solve(&auction).await;
        assert_eq!(solutions.len(), 1, "driver solution converted");
        let ranking = SolanaArbitrator::new(1, wrapped_native).arbitrate(solutions, &auction);
        assert_eq!(ranking.winner_count(), 1, "solution won");
    }

    let mut auction_loop = AuctionLoop::new(
        Box::new(FixedTrigger(tip)),
        Box::new(DbAuctionProvider::new(pool)),
        Box::new(DriverCompetition::new(
            vec![Arc::clone(&driver)],
            Duration::from_secs(6),
        )),
        Box::new(SolanaArbitrator::new(1, wrapped_native)),
        Box::new(DriverExecutor::new(vec![driver])),
        Box::new(LogObserver),
    );
    auction_loop.run_cycle().await;

    let settle = tokio::time::timeout(Duration::from_secs(5), settled.recv())
        .await
        .expect("settlement dispatched before the timeout")
        .expect("settle channel open");
    assert_eq!(settle.solution_id, 7);
    assert!(settle.auction_id > 0);
}
