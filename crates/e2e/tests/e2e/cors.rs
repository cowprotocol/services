//! Tests for CORS behavior to ensure the warp-to-axum migration preserves
//! headers.

use {
    e2e::setup::{API_HOST, OnchainComponents, Services, run_test},
    number::units::EthUnit,
    reqwest::{Method, StatusCode},
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_cors_preflight() {
    run_test(cors_preflight).await;
}

async fn cors_preflight(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    let response = client
        .request(Method::OPTIONS, format!("{API_HOST}/api/v1/orders"))
        .header("Origin", "https://swap.cow.fi")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Content-Type")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let headers = response.headers();

    let allow_methods = headers
        .get("access-control-allow-methods")
        .expect("Missing access-control-allow-methods header")
        .to_str()
        .unwrap();
    for method in ["GET", "POST", "DELETE", "OPTIONS", "PUT", "PATCH"] {
        assert!(
            allow_methods.contains(method),
            "access-control-allow-methods missing {method}: {allow_methods}"
        );
    }

    let allow_headers = headers
        .get("access-control-allow-headers")
        .expect("Missing access-control-allow-headers header")
        .to_str()
        .unwrap()
        .to_lowercase();
    for header in ["origin", "content-type", "x-auth-token", "x-appid"] {
        assert!(
            allow_headers.contains(header),
            "access-control-allow-headers missing {header}: {allow_headers}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn local_node_cors_headers_on_error() {
    run_test(cors_headers_on_error).await;
}

async fn cors_headers_on_error(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    let response = client
        .get(format!("{API_HOST}/api/v1/nonexistent"))
        .header("Origin", "https://example.com")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        response
            .headers()
            .contains_key("access-control-allow-origin"),
        "CORS headers should be present on 404 responses"
    );
}
