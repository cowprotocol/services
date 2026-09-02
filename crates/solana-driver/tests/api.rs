//! Integration tests for the HTTP API server.

use {
    cow_settlement_interface::{
        data::intent::{OrderIntent, OrderKind},
        pda::order::find_order_pda,
    },
    cow_solana_rpc::SolanaRPC,
    solana_driver::infra::{api::Api, blockchain::Solana, config, solver::Solver},
    solana_sdk::pubkey::Pubkey,
    solana_testlib::temp_keypair,
    std::{net::SocketAddr, num::NonZero, sync::Arc},
    tokio_util::sync::CancellationToken,
};

fn pubkey(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

/// Order intent used by the literal `/solve` request and the settle test.
fn test_order_intent() -> OrderIntent {
    OrderIntent {
        owner: pubkey(0x22),
        buy_token_account: pubkey(0x66),
        sell_token_account: pubkey(0x55),
        sell_amount: 1_000,
        buy_amount: 2_000,
        // Far future so the settle path's order-expiry check passes.
        valid_to: u32::MAX,
        kind: OrderKind::Sell,
        partially_fillable: false,
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
    let app = axum::Router::new().route(
        "/solve",
        axum::routing::post(move || {
            let response = response.clone();
            async move { axum::Json(response) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// A solver client whose on-chain identity is a freshly generated keypair,
/// so the test can register a matching settlement signer.
fn solver_with_keypair(addr: SocketAddr) -> (Solver, Pubkey) {
    let keypair_file = temp_keypair();
    let keypair_path = keypair_file.path().to_path_buf();
    let solver = Solver::new(&config::Solver {
        name: "mock".to_owned(),
        endpoint: format!("http://{addr}").parse().unwrap(),
        signer_keypair: keypair_path,
        max_in_flight: NonZero::new(1).unwrap(),
    })
    .expect("solver construction should succeed");
    let account = solver.pubkey();
    (solver, account)
}

/// A solver client pointing at a dead endpoint (no listener).
fn dead_solver() -> (Solver, Pubkey) {
    solver_with_keypair("127.0.0.1:1".parse().unwrap())
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
            "buyTokenAccount": pubkey(0x66).to_string(),
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
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/mock/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.unwrap()
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
            "score": "0",
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
            "score": "0",
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
