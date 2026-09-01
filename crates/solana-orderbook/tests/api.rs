//! Integration tests for the HTTP API server.

use {
    solana_orderbook::infra::{api::Api, quoter::Quoter},
    sqlx::PgPool,
    std::{net::SocketAddr, time::Duration},
    tokio_util::sync::CancellationToken,
};

fn mock_api() -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        // A lazy pool never connects unless queried, and `/healthz` does not
        // query, so the tests run without a database.
        pool: PgPool::connect_lazy("postgresql://").unwrap(),
        // Points at a dead endpoint: `/healthz` never quotes.
        quoter: Quoter::new(
            "http://127.0.0.1:1".parse().unwrap(),
            Duration::from_secs(1),
        ),
    }
}

/// Spawn the API server on an ephemeral port and return its bound address.
async fn spawn_server() -> SocketAddr {
    let api = mock_api();
    let (listener, addr) = api.bind().await.unwrap();
    // A token that is never cancelled keeps the server alive for the test.
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

#[tokio::test]
async fn healthz_returns_200() {
    let addr = spawn_server().await;
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn shuts_down_cleanly_on_signal() {
    let api = mock_api();
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

/// A malformed quote body is rejected in the API's error shape, not axum's
/// plain-text default.
#[tokio::test]
async fn malformed_quote_body_keeps_the_error_shape() {
    let addr = spawn_server().await;
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&serde_json::json!({"sellToken": "not-a-pubkey"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({
            "errorType": "InvalidRequestBody",
            "description": "The request body could not be parsed."
        })
    );
}

/// A quote body with the given validity fields, otherwise well formed.
fn quote_body(validity: serde_json::Value) -> serde_json::Value {
    let mut body = serde_json::json!({
        "from": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        "sellToken": "So11111111111111111111111111111111111111112",
        "buyToken": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "kind": "sell",
        "sellAmountBeforeFee": "10000000"
    });
    body.as_object_mut()
        .unwrap()
        .extend(validity.as_object().unwrap().clone());
    body
}

async fn post_quote(addr: SocketAddr, body: serde_json::Value) -> (reqwest::StatusCode, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let json: serde_json::Value = response.json().await.unwrap();
    (
        status,
        json["errorType"].as_str().unwrap_or_default().to_owned(),
    )
}

/// The order's validity is checked before any driver is asked, so these
/// answer with the validation error and never reach the dead quoter.
#[tokio::test]
async fn quote_validity_is_bounded() {
    let addr = spawn_server().await;
    let (status, kind) = post_quote(addr, quote_body(serde_json::json!({"validFor": 10}))).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InsufficientValidTo")
    );

    let (status, kind) = post_quote(
        addr,
        quote_body(serde_json::json!({"validFor": 4 * 60 * 60})),
    )
    .await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "ExcessiveValidTo")
    );
}
