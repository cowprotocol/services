//! Tests for malformed request handling to ensure error responses are preserved.

use {
    e2e::setup::{API_HOST, OnchainComponents, Services, run_test},
    number::units::EthUnit,
    orderbook::api::Error,
    reqwest::StatusCode,
    serde_json::json,
    shared::ethrpc::Web3,
};

const VALID_ORDER_UID: &str = "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
const VALID_ADDRESS: &str = "0x0000000000000000000000000000000000000001";

#[tokio::test]
#[ignore]
async fn local_node_malformed_order_uid() {
    run_test(malformed_order_uid).await;
}

async fn malformed_order_uid(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    let too_long_uid = format!("0x{}", "0".repeat(200));
    let non_hex_uid = format!("0x{}", "GG".repeat(56));

    let invalid_order_uids: Vec<(&str, &str)> = vec![
        ("0x1234", "too short"),
        (&too_long_uid, "too long"),
        (&non_hex_uid, "non-hex characters"),
        ("not-hex-at-all", "no hex prefix"),
    ];

    for (uid, description) in invalid_order_uids {
        let response = client
            .get(format!("{API_HOST}/api/v1/orders/{uid}"))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404 for invalid OrderUid ({description}): {uid}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn local_node_malformed_address() {
    run_test(malformed_address).await;
}

async fn malformed_address(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    let invalid_hex_addr = format!("0x{}", "G".repeat(40));

    let invalid_addresses: Vec<(&str, &str)> = vec![
        ("0x123", "too short"),
        ("not-an-address", "not hex format"),
        (&invalid_hex_addr, "invalid hex characters"),
    ];

    for (addr, description) in invalid_addresses {
        let response = client
            .get(format!("{API_HOST}/api/v1/account/{addr}/orders"))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404 for invalid Address ({description}): {addr}"
        );
    }

    for (addr, description) in [("0x123", "too short"), ("invalid", "not hex")] {
        let response = client
            .get(format!("{API_HOST}/api/v1/token/{addr}/native_price"))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404 for invalid token Address ({description}): {addr}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn local_node_malformed_tx_hash() {
    run_test(malformed_tx_hash).await;
}

async fn malformed_tx_hash(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    let invalid_hex_hash = format!("0x{}", "Z".repeat(64));

    let invalid_hashes: Vec<(&str, &str)> = vec![
        ("0x123", "too short"),
        ("invalid-hash", "not hex format"),
        (&invalid_hex_hash, "invalid hex characters"),
    ];

    for (hash, description) in invalid_hashes {
        let response = client
            .get(format!("{API_HOST}/api/v1/transactions/{hash}/orders"))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404 for invalid tx hash ({description}): {hash}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn local_node_malformed_auction_id() {
    run_test(malformed_auction_id).await;
}

async fn malformed_auction_id(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    let invalid_auction_ids: Vec<(&str, &str)> = vec![
        ("not-a-number", "non-numeric"),
        ("-1", "negative number"),
        ("99999999999999999999999", "u64 overflow"),
    ];

    for (id, description) in invalid_auction_ids {
        let response = client
            .get(format!("{API_HOST}/api/v1/solver_competition/{id}"))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404 for invalid AuctionId ({description}): {id}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn local_node_malformed_request_bodies() {
    run_test(malformed_request_bodies).await;
}

async fn malformed_request_bodies(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    // Invalid JSON syntax
    let response = client
        .post(format!("{API_HOST}/api/v1/orders"))
        .header("Content-Type", "application/json")
        .body("{invalid json}")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Invalid JSON syntax should return 400"
    );

    // Empty body returns 411 Length Required (valid HTTP semantics)
    let response = client
        .post(format!("{API_HOST}/api/v1/orders"))
        .header("Content-Type", "application/json")
        .body("")
        .send()
        .await
        .unwrap();

    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::LENGTH_REQUIRED,
        "Empty body should return 400 or 411, got {}",
        response.status()
    );

    // Missing required fields (empty object)
    let response = client
        .post(format!("{API_HOST}/api/v1/orders"))
        .header("Content-Type", "application/json")
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Missing required fields should return 400"
    );

    // Wrong field types
    let response = client
        .post(format!("{API_HOST}/api/v1/orders"))
        .header("Content-Type", "application/json")
        .json(&json!({
            "sellToken": "not-an-address",
            "buyToken": "also-not-an-address",
            "sellAmount": "not-a-number",
            "buyAmount": 12345,
            "validTo": "not-a-number",
            "kind": "sell",
            "signature": "0x"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Wrong field types should return 400"
    );

    // Invalid enum value
    let response = client
        .post(format!("{API_HOST}/api/v1/quote"))
        .header("Content-Type", "application/json")
        .json(&json!({
            "sellToken": VALID_ADDRESS,
            "buyToken": VALID_ADDRESS,
            "kind": "invalidKind",
            "from": VALID_ADDRESS
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Invalid enum value should return 400"
    );
}

#[tokio::test]
#[ignore]
async fn local_node_error_response_format() {
    run_test(error_response_format).await;
}

/// Deserialization errors return plain text, while business logic errors return JSON.
async fn error_response_format(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    // Deserialization errors return plain text with error description
    let response = client
        .post(format!("{API_HOST}/api/v1/orders"))
        .header("Content-Type", "application/json")
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body_text = response.text().await.unwrap();
    assert!(
        body_text.contains("deserialize error") || body_text.contains("missing field"),
        "Deserialization error should contain helpful description. Got: {body_text}"
    );

    // Business logic errors (e.g., order not found) return JSON format
    let response = client
        .get(format!("{API_HOST}/api/v1/orders/{VALID_ORDER_UID}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body_text = response.text().await.unwrap();
    let error: Error = serde_json::from_str(&body_text).unwrap_or_else(|e| {
        panic!("Failed to parse 404 response as Error: {e}. Body was: {body_text}")
    });

    assert!(
        !error.error_type.is_empty(),
        "Error response should have non-empty 'errorType' field"
    );
    assert!(
        !error.description.is_empty(),
        "Error response should have non-empty 'description' field"
    );
}
