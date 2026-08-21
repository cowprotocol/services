//! Integration tests for the HTTP API server.

use {
    cow_solana_rpc::SolanaRPC,
    solana_driver::infra::{api::Api, config, solver::Solver},
    solana_sdk::{
        pubkey::Pubkey,
        signer::{
            Signer,
            keypair::{Keypair, read_keypair_file},
        },
    },
    std::{net::SocketAddr, num::NonZero, path::PathBuf, sync::Arc},
    tokio_util::sync::CancellationToken,
};

fn pubkey(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

/// The autopilot's own literal order uid (32 bytes of `0x11`).
fn uid() -> String {
    format!("0x{}", "11".repeat(32))
}

/// Write a fresh keypair to a temp file and return its path.
fn temp_keypair() -> PathBuf {
    let file = tempfile::NamedTempFile::new().expect("create temp file");
    let path = file.into_temp_path().keep().expect("keep temp file");
    solana_sdk::signer::keypair::write_keypair_file(&Keypair::new(), &path).expect("write keypair");
    path
}

fn api_with(solvers: Vec<Solver>) -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        rpc: Arc::new(SolanaRPC::new_mock("succeeds".to_string())),
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
    let keypair_path = temp_keypair();
    let account = read_keypair_file(&keypair_path).unwrap().pubkey();
    let solver = Solver::new(&config::Solver {
        name: "mock".to_owned(),
        endpoint: format!("http://{addr}").parse().unwrap(),
        account,
        signer_keypair: keypair_path,
        max_in_flight: NonZero::new(1).unwrap(),
    })
    .expect("solver construction should succeed");
    (solver, account)
}

/// A solver client pointing at a dead endpoint (no listener).
fn dead_solver() -> (Solver, Pubkey) {
    solver_with_keypair("127.0.0.1:1".parse().unwrap())
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
            "sellAmount": "18446744073709551615",
            "buyAmount": "2000",
            "validTo": 42,
            "kind": "sell",
            "partiallyFillable": false,
            "orderPda": pubkey(0x77).to_string(),
        }]
    })
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
    assert_eq!(
        response.status(),
        reqwest::StatusCode::INTERNAL_SERVER_ERROR
    );

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
