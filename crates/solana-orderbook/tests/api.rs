//! Integration tests for the HTTP API server.

use {
    solana_orderbook::infra::api::Api,
    sqlx::PgPool,
    std::net::SocketAddr,
    tokio_util::sync::CancellationToken,
};

fn mock_api() -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        // A lazy pool never connects unless queried, and `/healthz` does not
        // query, so the tests run without a database.
        pool: PgPool::connect_lazy("postgresql://").unwrap(),
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
