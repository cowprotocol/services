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
        quoter: dead_quoter(),
    }
}

/// A quoter pointing at a dead endpoint: every quote attempt fails.
fn dead_quoter() -> Quoter {
    Quoter::new(
        "http://127.0.0.1:1".parse().unwrap(),
        Duration::from_secs(1),
    )
}

/// Spawn the API server on an ephemeral port and return its bound address.
async fn spawn_server() -> SocketAddr {
    spawn_server_with(dead_quoter()).await
}

/// Spawn the API server with the given quoter.
async fn spawn_server_with(quoter: Quoter) -> SocketAddr {
    let api = Api {
        quoter,
        ..mock_api()
    };
    let (listener, addr) = api.bind().await.unwrap();
    // A token that is never cancelled keeps the server alive for the test.
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

/// A tiny axum server that answers `/quote` with a fixed response. It stands
/// in for the driver.
async fn spawn_mock_driver(response: serde_json::Value) -> SocketAddr {
    let app = axum::Router::new().route(
        "/quote",
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

/// The full happy path: the driver's amounts come back in the EVM response
/// shape with the request's own fields echoed.
#[tokio::test]
async fn quote_answers_in_the_evm_shape() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1234567",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let addr = spawn_server_with(Quoter::new(
        format!("http://{driver}").parse().unwrap(),
        Duration::from_secs(1),
    ))
    .await;

    let valid_to = chrono::Utc::now().timestamp() + 600;
    let mut body = quote_body(serde_json::json!({"validTo": valid_to}));
    body["receiver"] = serde_json::json!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    body["appData"] = serde_json::json!(format!("0x{}", "11".repeat(32)));

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "quote": {
                "sellToken": body["sellToken"],
                "buyToken": body["buyToken"],
                "receiver": body["receiver"],
                "sellAmount": "10000000",
                "buyAmount": "1234567",
                "validTo": valid_to,
                "appData": body["appData"],
                "feeAmount": "0",
                "kind": "sell",
                "partiallyFillable": false,
            },
            "from": body["from"],
            "expiration": json["expiration"],
            "id": null,
            "verified": false,
        })
    );
}

/// Every driver failure reads as no liquidity, mirroring the EVM mapping of
/// estimator errors.
#[tokio::test]
async fn quote_without_a_route_is_no_liquidity() {
    let addr = spawn_server().await;
    let (status, kind) = post_quote(addr, quote_body(serde_json::json!({"validFor": 1800}))).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::NOT_FOUND, "NoLiquidity")
    );
}

#[tokio::test]
async fn quote_with_identical_tokens_is_rejected() {
    let addr = spawn_server().await;
    let mut body = quote_body(serde_json::json!({"validFor": 1800}));
    body["buyToken"] = body["sellToken"].clone();
    let (status, kind) = post_quote(addr, body).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "SameBuyAndSellToken")
    );
}

#[tokio::test]
async fn quote_of_a_zero_amount_is_rejected() {
    let addr = spawn_server().await;
    let mut body = quote_body(serde_json::json!({"validFor": 1800}));
    body["sellAmountBeforeFee"] = serde_json::json!("0");
    let (status, kind) = post_quote(addr, body).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "ZeroAmount")
    );
}
