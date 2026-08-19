//! Integration tests for the HTTP API server.

use {
    cow_solana_rpc::SolanaRPC,
    solana_driver::{
        domain::Competition,
        infra::{api::Api, config, solver::Solver},
    },
    solana_sdk::pubkey::Pubkey,
    std::{net::SocketAddr, num::NonZero},
    tokio_util::sync::CancellationToken,
};

fn pubkey(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

/// The autopilot's own literal order uid (32 bytes of `0x11`).
fn uid() -> String {
    format!("0x{}", "11".repeat(32))
}

fn api_with(competition: Competition) -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        rpc: SolanaRPC::new_mock("succeeds".to_string()),
        competition,
    }
}

/// Spawn the API server on an ephemeral port and return its bound address.
async fn spawn_server(competition: Competition) -> SocketAddr {
    let api = api_with(competition);
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

fn solver(addr: SocketAddr, account: Pubkey) -> Solver {
    Solver::new(&config::Solver {
        name: "mock".to_owned(),
        endpoint: format!("http://{addr}").parse().unwrap(),
        account,
        max_in_flight: NonZero::new(1).unwrap(),
    })
}

/// The autopilot's own literal `/solve` request JSON.
fn solve_request() -> serde_json::Value {
    serde_json::json!({
        "id": "7",
        "deadlineSlot": "100",
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
    let addr = spawn_server(Competition::new(Vec::new())).await;
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn shuts_down_cleanly_on_signal() {
    let api = api_with(Competition::new(Vec::new()));
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
    let account = pubkey(0x99);
    let engine = spawn_mock_solver_engine(serde_json::json!({
        "solutions": [{
            "id": 42,
            "trades": [{
                "orderUid": uid(),
                "executedAmount": "1000",
            }],
            "interactions": [],
        }]
    }))
    .await;
    let addr = spawn_server(Competition::new(vec![solver(engine, account)])).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    // The engine's id (42) is reindexed to 0, and the sell order's
    // side-matching amount fills `executedSell` while `executedBuy` is the
    // zero placeholder.
    let expected = serde_json::json!({
        "solutions": [{
            "solutionId": "0",
            "score": "0",
            "solver": account.to_string(),
            "orders": {
                (uid()): {
                    "executedSell": "1000",
                    "executedBuy": "0",
                }
            }
        }]
    });
    assert_eq!(json, expected);
}

#[tokio::test]
async fn solve_reindexes_colliding_engine_ids() {
    // Two engines both return a solution with the same engine-local id. The
    // driver reindexes them to driver-unique ids so the autopilot can address
    // each solution by (auction_id, solution_id) without collision.
    let account1 = pubkey(0x99);
    let account2 = pubkey(0x88);
    let engine1 = spawn_mock_solver_engine(serde_json::json!({
        "solutions": [{
            "id": 0,
            "trades": [{
                "orderUid": uid(),
                "executedAmount": "1000",
            }],
            "interactions": [],
        }]
    }))
    .await;
    let engine2 = spawn_mock_solver_engine(serde_json::json!({
        "solutions": [{
            "id": 0,
            "trades": [{
                "orderUid": uid(),
                "executedAmount": "500",
            }],
            "interactions": [],
        }]
    }))
    .await;
    let addr = spawn_server(Competition::new(vec![
        solver(engine1, account1),
        solver(engine2, account2),
    ]))
    .await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    let expected = serde_json::json!({
        "solutions": [
            {
                "solutionId": "0",
                "score": "0",
                "solver": account1.to_string(),
                "orders": {
                    (uid()): {
                        "executedSell": "1000",
                        "executedBuy": "0",
                    }
                }
            },
            {
                "solutionId": "1",
                "score": "0",
                "solver": account2.to_string(),
                "orders": {
                    (uid()): {
                        "executedSell": "500",
                        "executedBuy": "0",
                    }
                }
            }
        ]
    });
    assert_eq!(json, expected);
}

#[tokio::test]
async fn solve_with_engine_down_returns_solver_failed() {
    // Point the solver at a port with no listener.
    let dead = solver("127.0.0.1:1".parse().unwrap(), pubkey(0x99));
    let addr = spawn_server(Competition::new(vec![dead])).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "SolverFailed");
}

#[tokio::test]
async fn solve_with_partial_engine_failure_returns_successful_solutions() {
    // One engine returns a solution, the other is down. The failing engine
    // must not kill the auction: the driver returns the successful engine's
    // solution.
    let account = pubkey(0x99);
    let engine = spawn_mock_solver_engine(serde_json::json!({
        "solutions": [{
            "id": 0,
            "trades": [{
                "orderUid": uid(),
                "executedAmount": "1000",
            }],
            "interactions": [],
        }]
    }))
    .await;
    let dead = solver("127.0.0.1:1".parse().unwrap(), pubkey(0x88));
    let addr = spawn_server(Competition::new(vec![solver(engine, account), dead])).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/solve"))
        .json(&solve_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    let solutions = json["solutions"].as_array().unwrap();
    assert_eq!(solutions.len(), 1);
    assert_eq!(solutions[0]["solutionId"], "0");
}

#[tokio::test]
async fn settle_rejects_non_positive_auction_id() {
    let addr = spawn_server(Competition::new(Vec::new())).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/settle"))
        .json(&serde_json::json!({ "auctionId": "0", "solutionId": "3" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["kind"], "InvalidAuctionId");
}
