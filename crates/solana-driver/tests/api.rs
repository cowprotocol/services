//! Integration tests for the HTTP API server.

use {
    cow_settlement_interface::{
        data::intent::{Asset, Flags, OrderIntent, OrderKind, TokenAsset},
        pda::order::find_order_pda,
    },
    cow_solana_rpc::{Mocks, RpcRequest, SolanaRPC},
    solana_driver::{
        domain::solver_fee::SolverFee,
        infra::{
            api::Api,
            blockchain::{Solana, associated_token_address},
            config,
            solver::Solver,
        },
    },
    solana_sdk::pubkey::Pubkey,
    solana_testlib::temp_keypair,
    std::{
        net::SocketAddr,
        num::NonZero,
        sync::{Arc, Mutex},
    },
    tokio_util::sync::CancellationToken,
};

fn pubkey(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

/// Order intent used by the literal `/solve` request and the settle test.
/// The buy token account is the owner's associated token account, the one
/// the settlement creates when the mock RPC answers "absent".
fn test_order_intent() -> OrderIntent {
    OrderIntent {
        owner: pubkey(0x22),
        sell: TokenAsset {
            mint: pubkey(0x33),
            token_account: pubkey(0x55),
        },
        buy: Asset::TokenProgram(TokenAsset {
            mint: pubkey(0x44),
            token_account: buy_token_account(),
        }),
        sell_amount: 1_000,
        buy_amount: 2_000,
        // Far future so the settle path's order-expiry check passes.
        valid_to: u32::MAX,
        flags: Flags {
            created_on_chain: true,
            kind: OrderKind::Sell,
            partially_fillable: false,
        },
        app_data: [0; 32],
    }
}

/// The autopilot's own literal order uid, derived from the canonical order
/// intent so it passes settlement validation.
fn uid() -> String {
    format!(
        "0x{}",
        const_hex::encode(test_order_intent().uid().to_bytes())
    )
}

fn buy_token_account() -> Pubkey {
    associated_token_address(&pubkey(0x22), &pubkey(0x44))
}

fn blockchain() -> Arc<Solana> {
    Arc::new(Solana::new(
        SolanaRPC::new_mock("succeeds".to_string()),
        cow_settlement_interface::id(),
    ))
}

fn api_with(solvers: Vec<Solver>) -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        blockchain: blockchain(),
        solvers,
    }
}

/// Spawn the API server on an ephemeral port and return its bound address.
async fn spawn_server(solvers: Vec<Solver>) -> SocketAddr {
    let api = api_with(solvers);
    let (listener, addr) = api.bind().await.unwrap();
    // The test never cancels this token, so the server stays alive.
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

/// A tiny axum server that returns a fixed `/solve` response. It stands in
/// for a solver engine.
async fn spawn_mock_solver_engine(response: serde_json::Value) -> SocketAddr {
    spawn_recording_solver_engine(response).await.0
}

/// A mock solver engine that also records the last `/solve` request body it
/// received.
async fn spawn_recording_solver_engine(
    response: serde_json::Value,
) -> (SocketAddr, Arc<Mutex<Option<serde_json::Value>>>) {
    let requests = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&requests);
    let app = axum::Router::new().route(
        "/solve",
        axum::routing::post(move |axum::Json(request): axum::Json<serde_json::Value>| {
            let response = response.clone();
            *recorded.lock().unwrap() = Some(request);
            async move { axum::Json(response) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (addr, requests)
}

/// A solver client whose on-chain identity is a freshly generated keypair,
/// so the test can register a matching settlement signer.
fn solver_with_keypair(addr: SocketAddr) -> (Solver, Pubkey) {
    solver_with_fee(addr, 0)
}

fn solver_with_fee(addr: SocketAddr, solver_fee_bps: u16) -> (Solver, Pubkey) {
    let keypair_file = temp_keypair();
    let keypair_path = keypair_file.path().to_path_buf();
    let solver = Solver::new(&config::Solver {
        name: "mock".to_owned(),
        endpoint: format!("http://{addr}").parse().unwrap(),
        signer_keypair: keypair_path,
        solve_every_nth_auction: None,
        solver_fee_bps: (solver_fee_bps > 0).then(|| SolverFee::try_from(solver_fee_bps).unwrap()),
    })
    .expect("solver construction should succeed");
    let account = solver.pubkey();
    (solver, account)
}

/// A solver client pointing at a dead endpoint (no listener).
fn dead_solver() -> (Solver, Pubkey) {
    solver_with_keypair("127.0.0.1:1".parse().unwrap())
}

/// A dead-endpoint solver throttled to one solve in the given stride.
fn throttled_dead_solver(stride: u64) -> Solver {
    let keypair_file = temp_keypair();
    Solver::new(&config::Solver {
        name: "mock".to_owned(),
        endpoint: "http://127.0.0.1:1".parse().unwrap(),
        signer_keypair: keypair_file.path().to_path_buf(),
        solve_every_nth_auction: NonZero::new(stride),
        solver_fee_bps: None,
    })
    .expect("solver construction should succeed")
}

fn order_pda() -> Pubkey {
    find_order_pda(&cow_settlement_interface::id(), &test_order_intent().uid()).0
}

/// The autopilot's own literal `/solve` request JSON.
///
/// The deadline is computed relative to now so the request is always
/// solvable, regardless of when the test runs.
fn solve_request() -> serde_json::Value {
    let deadline = chrono::Utc::now() + chrono::Duration::minutes(5);
    serde_json::json!({
        "id": 7,
        "deadline": deadline.to_rfc3339(),
        "orders": [{
            "uid": uid(),
            "owner": pubkey(0x22).to_string(),
            "sellToken": pubkey(0x33).to_string(),
            "buyToken": pubkey(0x44).to_string(),
            "sellTokenAccount": pubkey(0x55).to_string(),
            "buyTokenAccount": buy_token_account().to_string(),
            "sellAmount": "1000",
            "buyAmount": "2000",
            "validTo": u32::MAX,
            "kind": "sell",
            "partiallyFillable": false,
            "orderPda": order_pda().to_string(),
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
        }]
    })
}

/// A solver-engine `/solve` response with one solution per `(id,
/// sell_price)` pair. The `sell_price` sets the clearing price of the sell
/// mint, so `executedBuy` identifies which payload survives an id collision.
fn engine_response(solutions: &[(u64, &str)]) -> serde_json::Value {
    let solutions: Vec<serde_json::Value> = solutions
        .iter()
        .map(|(id, sell_price)| {
            serde_json::json!({
                "id": id,
                "prices": {
                    (pubkey(0x33).to_string()): sell_price,
                    (pubkey(0x44).to_string()): "1000",
                },
                "trades": [{
                    "orderUid": uid(),
                    "executedAmount": "1000",
                }],
                "interactions": [],
            })
        })
        .collect();
    serde_json::json!({ "solutions": solutions })
}

/// POST the standard solve request and return the parsed response body.
async fn call_solve(addr: SocketAddr) -> serde_json::Value {
    call_solve_with(addr, solve_request()).await
}

async fn call_solve_with(addr: SocketAddr, request: serde_json::Value) -> serde_json::Value {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/solve"))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.unwrap()
}

/// Post a `/solve` and return the HTTP status, without asserting on it.
async fn solve_status(addr: SocketAddr) -> reqwest::StatusCode {
    reqwest::Client::new()
        .post(format!("http://{addr}/mock/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap()
        .status()
}

/// The solution ids in a `/solve` response body, in response order.
fn response_ids(body: &serde_json::Value) -> Vec<u64> {
    body["solutions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|solution| solution["solutionId"].as_u64().unwrap())
        .collect()
}

#[tokio::test]
async fn healthz_returns_200() {
    let addr = spawn_server(Vec::new()).await;
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn shuts_down_cleanly_on_signal() {
    let api = api_with(Vec::new());
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown_token = CancellationToken::new();
    let serve = api.serve(listener, shutdown_token.clone());
    let handle = tokio::spawn(async move { serve.await.unwrap() });

    // The server is up and serving requests.
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // Trigger graceful shutdown and assert the serve future completes cleanly.
    shutdown_token.cancel();
    handle.await.unwrap();
}

#[tokio::test]
async fn solve_returns_converted_solutions() {
    let engine = spawn_mock_solver_engine(serde_json::json!({
        "solutions": [{
            "id": 42,
            "prices": {
                (pubkey(0x33).to_string()): "2000",
                (pubkey(0x44).to_string()): "1000",
            },
            "trades": [{
                "orderUid": uid(),
                "executedAmount": "1000",
            }],
            "interactions": [],
        }]
    }))
    .await;
    let (solver, account) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    // The sell order's side-matching amount fills `executedSell` and the
    // counterpart leg is derived from the clearing prices.
    let expected = serde_json::json!({
        "solutions": [{
            "solutionId": 42,
            "solver": account.to_string(),
            "orders": {
                (uid()): {
                    "executedSell": "1000",
                    "executedBuy": "2000",
                }
            }
        }]
    });
    assert_eq!(json, expected);
}

/// The default mock RPC answers every account lookup with "absent", so the
/// order's buy token account is flagged for creation on the way to the engine.
#[tokio::test]
async fn solve_flags_a_missing_buy_token_account_to_the_engine() {
    let (engine, requests) = spawn_recording_solver_engine(engine_response(&[(1, "2000")])).await;
    let (solver, _) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let body = call_solve(addr).await;
    assert_eq!(response_ids(&body), vec![1]);

    let request = requests.lock().unwrap().take().unwrap();
    assert_eq!(
        request["orders"][0]["missingBuyTokenAccount"],
        serde_json::json!(true)
    );
}

/// An absent buy token account that is not the owner's associated token
/// account is nothing the settlement can create, so the engine is not told
/// to price its rent. The order still reaches the engine: dropping
/// unreceivable orders is the autopilot's cut.
#[tokio::test]
async fn solve_does_not_flag_an_absent_account_it_cannot_create() {
    let (engine, requests) = spawn_recording_solver_engine(engine_response(&[(1, "2000")])).await;
    let (solver, _) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;
    let mut request = solve_request();
    request["orders"][0]["buyTokenAccount"] = serde_json::json!(pubkey(0x66).to_string());

    let body = call_solve_with(addr, request).await;
    assert_eq!(response_ids(&body), vec![1]);

    let request = requests.lock().unwrap().take().unwrap();
    assert!(request["orders"][0].get("missingBuyTokenAccount").is_none());
}

/// Two solutions with the same id: the driver keeps only the last occurrence
/// (each `HashMap::insert` replaces the earlier entry), because
/// the id is the handle `/settle` addresses a solution by.
#[tokio::test]
async fn solve_discards_duplicate_solution_ids() {
    let solution = serde_json::json!({
        "id": 42,
        "prices": {
            (pubkey(0x33).to_string()): "2000",
            (pubkey(0x44).to_string()): "1000",
        },
        "trades": [{
            "orderUid": uid(),
            "executedAmount": "1000",
        }],
        "interactions": [],
    });
    let engine = spawn_mock_solver_engine(serde_json::json!({
        "solutions": [solution.clone(), solution],
    }))
    .await;
    let (solver, _) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["solutions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn solve_with_engine_down_returns_solver_failed() {
    // Point the solver at a port with no listener.
    let (dead, _) = dead_solver();
    let addr = spawn_server(vec![dead]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "SolverFailed");
}

#[tokio::test]
async fn settle_rejects_non_positive_auction_id() {
    let (dead, _) = dead_solver();
    let addr = spawn_server(vec![dead]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/settle"))
        .json(
            &serde_json::json!({ "auctionId": 0, "solutionId": 3, "submissionDeadlineSlot": 125 }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "InvalidAuctionId");
}

#[tokio::test]
async fn settle_rejects_a_passed_submission_deadline() {
    let engine = spawn_mock_solver_engine(engine_response(&[(42, "2000")])).await;
    let (solver, _) = solver_with_keypair(engine);

    // The mock RPC reports slot 1000, so a deadline of 500 is already past.
    let mut mocks = Mocks::new();
    mocks.insert(RpcRequest::GetSlot, serde_json::json!(1000));
    let blockchain = Arc::new(Solana::new(
        SolanaRPC::new_mock_with_mocks(mocks),
        cow_settlement_interface::id(),
    ));

    let api = Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        blockchain,
        solvers: vec![solver],
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });

    // Populate the cache with a solution for auction 7.
    let body = call_solve(addr).await;
    let solution_id = body["solutions"][0]["solutionId"].as_u64().unwrap();

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/settle"))
        .json(&serde_json::json!({
            "auctionId": 7,
            "solutionId": solution_id,
            "submissionDeadlineSlot": 500,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "DeadlineExceeded");
}

#[tokio::test]
async fn solve_keeps_the_first_of_duplicate_solution_ids() {
    // Both solutions use id 7 but different sell prices. `executedBuy`
    // identifies the survivor: 1000 * 2000 / 1000 = 2000 for the first one.
    let engine = spawn_mock_solver_engine(engine_response(&[(7, "2000"), (7, "4000")])).await;
    let (solver, account) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let body = call_solve(addr).await;
    let expected = serde_json::json!({
        "solutions": [{
            "solutionId": 7,
            "solver": account.to_string(),
            "orders": {
                (uid()): {
                    "executedSell": "1000",
                    "executedBuy": "2000",
                }
            }
        }]
    });
    assert_eq!(body, expected);
}

#[tokio::test]
async fn solve_preserves_the_solvers_ordering() {
    let engine =
        spawn_mock_solver_engine(engine_response(&[(3, "2000"), (1, "2000"), (2, "2000")])).await;
    let (solver, _) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let body = call_solve(addr).await;
    assert_eq!(response_ids(&body), vec![3, 1, 2]);
}

/// A `/quote` request body for the given side.
fn quote_request(kind: &str, amount: &str) -> serde_json::Value {
    let deadline = chrono::Utc::now() + chrono::Duration::minutes(5);
    serde_json::json!({
        "sellToken": pubkey(0x33).to_string(),
        "buyToken": pubkey(0x44).to_string(),
        "amount": amount,
        "kind": kind,
        "deadline": deadline.to_rfc3339(),
    })
}

/// A canned engine solution for the fabricated quote order, whose uid is
/// zero.
fn quote_solution(executed_amount: &str) -> serde_json::Value {
    serde_json::json!({
        "solutions": [{
            "id": 42,
            "prices": {
                (pubkey(0x33).to_string()): "2000",
                (pubkey(0x44).to_string()): "1000",
            },
            "trades": [{
                "orderUid": format!("0x{}", "00".repeat(32)),
                "executedAmount": executed_amount,
            }],
            "interactions": [],
        }]
    })
}

#[tokio::test]
async fn quote_returns_the_executed_amounts() {
    let engine = spawn_mock_solver_engine(quote_solution("1000")).await;
    let (solver, account) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/quote"))
        .json(&quote_request("sell", "1000"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "sellAmount": "1000",
            "buyAmount": "2000",
            "solver": account.to_string(),
        })
    );
}

#[tokio::test]
async fn buy_quote_returns_the_executed_amounts() {
    let engine = spawn_mock_solver_engine(quote_solution("2000")).await;
    let (solver, account) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/quote"))
        .json(&quote_request("buy", "2000"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "sellAmount": "1000",
            "buyAmount": "2000",
            "solver": account.to_string(),
        })
    );
}

/// The auction order sells 1000 for at least 2000. A route paying 3000 is
/// reported at 2850 after a 500 bps fee.
#[tokio::test]
async fn solve_reports_fee_adjusted_amounts() {
    let engine = spawn_mock_solver_engine(engine_response(&[(1, "3000")])).await;
    let (solver, _) = solver_with_fee(engine, 500);
    let addr = spawn_server(vec![solver]).await;

    let body = call_solve(addr).await;
    let amounts = &body["solutions"][0]["orders"][uid()];
    assert_eq!(amounts["executedSell"], "1000");
    assert_eq!(amounts["executedBuy"], "2850");
}

/// A route paying exactly the 2000 limit undercuts it once the fee applies
/// and is dropped before the autopilot sees it. The other solution survives.
#[tokio::test]
async fn solve_drops_the_fill_the_fee_pushes_under_the_limit() {
    let engine = spawn_mock_solver_engine(engine_response(&[(1, "2000"), (2, "3000")])).await;
    let (solver, _) = solver_with_fee(engine, 500);
    let addr = spawn_server(vec![solver]).await;

    let body = call_solve(addr).await;
    assert_eq!(response_ids(&body), vec![2]);
}

#[tokio::test]
async fn sell_quote_reports_the_fee_adjusted_buy_amount() {
    let engine = spawn_mock_solver_engine(quote_solution("1000")).await;
    let (solver, account) = solver_with_fee(engine, 500);
    let addr = spawn_server(vec![solver]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/quote"))
        .json(&quote_request("sell", "1000"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "sellAmount": "1000",
            "buyAmount": "1900",
            "solver": account.to_string(),
        })
    );
}

/// A buy quote pulls `1000 * 1.05 = 1050` sell units after a 500 bps fee; the
/// quote order's unbounded sell limit does not overflow the check.
#[tokio::test]
async fn buy_quote_reports_the_fee_adjusted_sell_amount() {
    let engine = spawn_mock_solver_engine(quote_solution("2000")).await;
    let (solver, account) = solver_with_fee(engine, 500);
    let addr = spawn_server(vec![solver]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/quote"))
        .json(&quote_request("buy", "2000"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "sellAmount": "1050",
            "buyAmount": "2000",
            "solver": account.to_string(),
        })
    );
}

#[tokio::test]
async fn quote_with_identical_tokens_is_rejected() {
    // Validation short-circuits before the engine is called.
    let (solver, _) = dead_solver();
    let addr = spawn_server(vec![solver]).await;

    let mut body = quote_request("sell", "1000");
    body["buyToken"] = serde_json::json!(pubkey(0x33).to_string());
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "QuoteSameTokens");
}

#[tokio::test]
async fn quote_without_a_solution_is_quoting_failed() {
    // An engine that routes nothing answers with an empty solution list.
    let engine = spawn_mock_solver_engine(serde_json::json!({"solutions": []})).await;
    let (solver, _) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/quote"))
        .json(&quote_request("sell", "1000"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "QuotingFailed");
}

/// A quoted solution must not become settleable: quoting leaves the solution
/// cache untouched, so any later `/settle` misses.
#[tokio::test]
async fn quoting_does_not_populate_the_settle_cache() {
    let engine = spawn_mock_solver_engine(quote_solution("1000")).await;
    let (solver, _) = solver_with_keypair(engine);
    let addr = spawn_server(vec![solver]).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{addr}/mock/quote"))
        .json(&quote_request("sell", "1000"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let response = client
        .post(format!("http://{addr}/mock/settle"))
        .json(&serde_json::json!({
            "auctionId": 1,
            "solutionId": 42,
            "submissionDeadlineSlot": 100,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "SolutionNotAvailable");
}

/// A throttled solver takes part in one solve out of every N, counting the
/// solves it receives rather than the auction id. Off the stride the driver
/// answers an empty solution set without asking the engine, on the stride the
/// request reaches the (dead) engine and fails.
#[tokio::test]
async fn solve_takes_part_every_nth_solve() {
    let addr = spawn_server(vec![throttled_dead_solver(2)]).await;

    // First solve (seq 0) takes part: it reaches the dead engine and fails.
    assert_eq!(solve_status(addr).await, reqwest::StatusCode::BAD_REQUEST);

    // Second solve (seq 1) sits out with an empty solution set.
    let body = call_solve(addr).await;
    assert_eq!(body["solutions"].as_array().unwrap().len(), 0);

    // Third solve (seq 2) takes part again, one full stride later.
    assert_eq!(solve_status(addr).await, reqwest::StatusCode::BAD_REQUEST);
}
