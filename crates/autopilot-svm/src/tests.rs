//! The BE-184 checkpoint: one full auction cycle through [`AuctionLoop`]
//! against the real database and a mocked driver.

use {
    crate::{
        domain::{arbitrator::SolanaArbitrator, cycle::SolanaCycle},
        infra::{
            competition::DriverCompetition,
            driver::{Driver, dto},
            executor::DriverExecutor,
            observation::SettlementWindows,
            observer::CompetitionObserver,
            prices::NativePrices,
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
    base64::{Engine, prelude::BASE64_STANDARD},
    chain_types::solana::{IntentHash, Pubkey, Signature},
    cow_solana_rpc::{Mocks, RpcRequest, SolanaRPC},
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

/// A canned `getMultipleAccounts` entry: an initialized mint of the classic
/// SPL token program with the given decimals.
pub(crate) fn mint_account_json(decimals: u8) -> serde_json::Value {
    let mut data = [0u8; 82];
    data[44] = decimals;
    // The initialized flag.
    data[45] = 1;
    serde_json::json!({
        "lamports": 1_461_600u64,
        "data": [BASE64_STANDARD.encode(data), "base64"],
        "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "executable": false,
        "rentEpoch": 0u64,
        "space": 82u64,
    })
}

/// A canned `getMultipleAccounts` entry: an initialized account of the
/// classic SPL token program holding `mint`.
pub(crate) fn token_account_json(mint: [u8; 32]) -> serde_json::Value {
    let mut data = [0u8; 165];
    data[..32].copy_from_slice(&mint);
    // The account state byte: 1 is Initialized.
    data[108] = 1;
    serde_json::json!({
        "lamports": 2_039_280u64,
        "data": [BASE64_STANDARD.encode(data), "base64"],
        "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "executable": false,
        "rentEpoch": 0u64,
        "space": 165u64,
    })
}

/// A mock RPC answering one buy-account lookup with an initialized token
/// account of the seeded order's buy mint. Each canned response serves once,
/// later cuts fail open and keep the orders.
fn mock_rpc() -> SolanaRPC {
    let response = serde_json::json!({
        "context": {"slot": 1u64, "apiVersion": "2.0.0"},
        "value": [token_account_json([0xAB; 32])],
    });
    SolanaRPC::new_mock_with_mocks(Mocks::from([(RpcRequest::GetMultipleAccounts, response)]))
}

/// Native prices for the seeded order's pair, at the denominator so scores
/// stay raw surplus.
fn test_prices() -> [(solana_sdk::pubkey::Pubkey, u64); 2] {
    [
        (
            solana_sdk::pubkey::Pubkey::new_from_array([0xAA; 32]),
            1_000_000_000,
        ),
        (
            solana_sdk::pubkey::Pubkey::new_from_array([0xAB; 32]),
            1_000_000_000,
        ),
    ]
}

async fn seed_open_order(pool: &PgPool, uid: [u8; 32], tip: i64) {
    crate::test_db::wipe(pool).await;
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
VALUES ($1, $2, $2, $3, $2, $2, 1000, 500, $4, 'sell'::solana.OrderKind, false, $2, now(), $5)
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
    let pool = crate::test_db::pool().await;
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
        let provider = DbAuctionProvider::new(
            pool.clone(),
            mock_rpc(),
            NativePrices::seeded(test_prices()),
        );
        let auction = provider.cut_auction(&tip).await.expect("auction cut");
        assert_eq!(auction.orders.len(), 1, "open order in the auction");
        let competition = DriverCompetition::new(vec![Arc::clone(&driver)], Duration::from_secs(6));
        let solutions = competition.solve(&auction).await;
        assert_eq!(solutions.len(), 1, "driver solution converted");
        let ranking = SolanaArbitrator::new(1, wrapped_native).arbitrate(solutions, &auction);
        assert_eq!(ranking.winner_count(), 1, "solution won");
    }

    let windows = SettlementWindows::new(pool.clone());
    let mut auction_loop = AuctionLoop::new(
        Box::new(FixedTrigger(tip)),
        Box::new(DbAuctionProvider::new(
            pool.clone(),
            mock_rpc(),
            NativePrices::seeded(test_prices()),
        )),
        Box::new(DriverCompetition::new(
            vec![Arc::clone(&driver)],
            Duration::from_secs(6),
        )),
        Box::new(SolanaArbitrator::new(1, wrapped_native)),
        Box::new(DriverExecutor::new(vec![driver], windows.clone(), None)),
        Box::new(CompetitionObserver::new(pool.clone(), windows.clone())),
        25,
    );
    auction_loop.run_cycle().await;

    let settle = tokio::time::timeout(Duration::from_secs(5), settled.recv())
        .await
        .expect("settlement dispatched before the timeout")
        .expect("settle channel open");
    assert_eq!(settle.solution_id, 7);
    assert!(settle.auction_id > 0);
    // The dispatch opened a settlement-execution window.
    let open_windows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM solana.settlement_executions WHERE outcome IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(open_windows, 1);
    // The competition was persisted: the snapshot row, the proposed solution
    // under its generated uid, its execution, and the window keyed by the
    // same uid.
    let snapshots: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.competition_auctions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(snapshots, 1);
    let (solution_uid, solver_id, is_winner): (i64, i64, bool) =
        sqlx::query_as("SELECT uid, id, is_winner FROM solana.proposed_solutions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((solution_uid, solver_id, is_winner), (0, 7, true));
    let (executed_sell, executed_buy): (String, String) = sqlx::query_as(
        "SELECT executed_sell::text, executed_buy::text FROM solana.proposed_trade_executions \
         WHERE solution_uid = 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (executed_sell.as_str(), executed_buy.as_str()),
        ("1000", "600")
    );
    let window_uid: i64 =
        sqlx::query_scalar("SELECT solution_uid FROM solana.settlement_executions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(window_uid, 0);
    // The cycle reported the order's auction progress. The writes are detached
    // from the cycle, so they can land after `run_cycle` returns.
    let events = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let mut events: Vec<String> =
                sqlx::query_scalar("SELECT label::text FROM solana.order_events")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            if events.len() == 2 {
                events.sort();
                return events;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("order events written before the timeout");
    assert_eq!(events, ["executing", "ready"]);
}
