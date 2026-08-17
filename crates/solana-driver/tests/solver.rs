//! Integration tests for the solver-engine HTTP client.
//!
//! These tests mock the external solver engine with an in-process TCP stub so
//! the driver <-> solver wire boundary is exercised end to end.

use {
    base64::Engine,
    solana_driver::{
        domain::{Auction, Order, Side, order_uid::OrderUid},
        infra::{config, solver::Solver},
    },
    solana_sdk::pubkey::Pubkey,
    std::{net::SocketAddr, time::Duration},
    tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    },
    tokio_util::sync::CancellationToken,
};

fn pubkey(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn sample_auction() -> Auction {
    Auction {
        id: 7,
        orders: vec![Order {
            uid: OrderUid([8; 32]),
            sell_mint: pubkey(1),
            buy_mint: pubkey(2),
            amount: 1_000,
            side: Side::Sell,
        }],
    }
}

fn solver_config(endpoint: String, timeout: Duration) -> config::Solver {
    config::Solver {
        name: "stub".to_string(),
        endpoint: endpoint.parse().unwrap(),
        account: pubkey(6),
        timeout,
        max_in_flight: std::num::NonZero::new(1).unwrap(),
    }
}

/// Spawn a minimal TCP server that replies to a single POST /solve with the
/// given body (after the optional delay) and then shuts down.
async fn spawn_engine(body: String, delay: Option<Duration>) -> (SocketAddr, CancellationToken) {
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let token = shutdown.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                Ok((mut stream, _)) = listener.accept() => {
                    let mut buf = [0u8; 2048];
                    // Read the request headers (we don't validate them).
                    let _ = stream.read(&mut buf).await;
                    if let Some(delay) = delay {
                        tokio::time::sleep(delay).await;
                    }
                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.flush().await;
                }
            }
        }
    });
    (addr, shutdown)
}

#[tokio::test]
async fn happy_path_maps_wire_solution_to_domain() {
    let body = serde_json::json!({
        "solutions": [{
            "id": 1,
            "trades": [{
                "orderUid": format!("0x{}", "08".repeat(32)),
                "executedAmount": "1000",
            }],
            "interactions": [{
                "programId": pubkey(9).to_string(),
                "accounts": [{
                    "pubkey": pubkey(4).to_string(),
                    "isSigner": true,
                    "isWritable": false,
                }],
                "instructionData": Engine::encode(&base64::prelude::BASE64_STANDARD,
                    [0xde, 0xad]
                ),
            }],
            "addressLookupTables": [pubkey(7).to_string()],
        }]
    })
    .to_string();

    let (addr, shutdown) = spawn_engine(body, None).await;
    let solver = Solver::new(&solver_config(
        format!("http://{addr}"),
        Duration::from_secs(30),
    ));

    let solutions = solver.solve(&sample_auction()).await.unwrap();

    assert_eq!(solutions.len(), 1);
    assert_eq!(solutions[0].id, 1);
    assert_eq!(solutions[0].solver, pubkey(6));
    assert_eq!(solutions[0].trades[0].order_uid, OrderUid([8; 32]));
    assert_eq!(solutions[0].trades[0].executed_amount, 1_000);
    assert_eq!(solutions[0].interactions[0].program_id, pubkey(9));

    shutdown.cancel();
}

#[tokio::test]
async fn slow_engine_times_out() {
    let body = serde_json::json!({ "solutions": [] }).to_string();
    // Give the solver a very short timeout and make the stub sleep for a second.
    // The reqwest times out before the stub responds.
    let (addr, shutdown) = spawn_engine(body, Some(Duration::from_secs(1))).await;
    let solver = Solver::new(&solver_config(
        format!("http://{addr}"),
        Duration::from_millis(10),
    ));

    let err = solver.solve(&sample_auction()).await.unwrap_err();

    assert!(matches!(
        err,
        solana_driver::infra::solver::Error::Timeout | solana_driver::infra::solver::Error::Http(_)
    ));
    shutdown.cancel();
}

#[tokio::test]
async fn unknown_order_uid_returns_no_solutions() {
    // The solver client returns an error for unknown UIDs; `solve_all` swallows
    // per-engine errors and contributes nothing to the merged result.
    let body = serde_json::json!({
        "solutions": [{
            "id": 1,
            "trades": [{
                "orderUid": format!("0x{}", "ff".repeat(32)),
                "executedAmount": "1000",
            }],
            "interactions": [],
            "addressLookupTables": [],
        }]
    })
    .to_string();

    let (addr, shutdown) = spawn_engine(body, None).await;
    let solver = Solver::new(&solver_config(
        format!("http://{addr}"),
        Duration::from_secs(30),
    ));

    let err = solver.solve(&sample_auction()).await.unwrap_err();
    assert!(
        format!("{err}").contains("unknown order UID"),
        "unexpected error: {err}"
    );

    shutdown.cancel();
}
