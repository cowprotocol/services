//! Tests for malformed request handling and missing endpoints to ensure error
//! responses are preserved.

use {
    configs::{
        order_quoting::{ExternalSolver, OrderQuoting},
        shared::SharedConfig,
        test_util::TestDefault,
    },
    e2e::setup::{API_HOST, OnchainComponents, Services, run_test},
    model::order::{ORDER_UID_LIMIT, OrderUid},
    orderbook::api::Error,
    reqwest::StatusCode,
    serde_json::json,
    shared::web3::Web3,
};

const VALID_ORDER_UID: &str = "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
const VALID_ADDRESS: &str = "0x0000000000000000000000000000000000000001";

#[tokio::test]
#[ignore]
async fn local_node_http_validation() {
    run_test(http_validation).await;
}

/// HTTP validation test covering malformed parameters, request
/// bodies, missing endpoints, and error response formats.
async fn http_validation(web3: Web3) {
    let onchain = OnchainComponents::deploy(web3).await;
    let services = Services::new(&onchain).await;
    // since we're testing malformed paths, etc;
    // we don't really need the rest of the protocol
    services
        .start_api(configs::orderbook::Configuration {
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "test_quoter",
                "http://localhost:11088/test_solver",
            )]),
            shared: SharedConfig {
                gas_estimators: vec![TestDefault::test_default()],
                ..Default::default()
            },
            ..configs::orderbook::Configuration::test_default()
        })
        .await;
    let client = services.client();

    // Test malformed order UIDs
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
            StatusCode::BAD_REQUEST,
            "Expected 400 for invalid OrderUid ({description}): {uid}"
        );
    }

    // Test malformed addresses
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
            StatusCode::BAD_REQUEST,
            "Expected 400 for invalid Address ({description}): {addr}"
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
            StatusCode::BAD_REQUEST,
            "Expected 400 for invalid token Address ({description}): {addr}"
        );
    }

    // Test malformed transaction hashes
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
            StatusCode::BAD_REQUEST,
            "Expected 400 for invalid tx hash ({description}): {hash}"
        );
    }

    // Test malformed auction IDs
    for (id, description, expected_status) in [
        ("not-a-number", "non-numeric", StatusCode::BAD_REQUEST),
        ("-1", "negative number", StatusCode::BAD_REQUEST),
        (
            "99999999999999999999999",
            "u64 overflow",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = client
            .get(format!("{API_HOST}/api/v1/solver_competition/{id}"))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            expected_status,
            "Expected {expected_status} for invalid AuctionId ({description}): {id}"
        );
    }

    // Test missing/invalid endpoints
    let extra_segment_path = format!("/api/v1/orders/{VALID_ORDER_UID}/extra");
    let wrong_nested_path = format!("/api/v1/account/{VALID_ADDRESS}/trades");

    let missing_endpoint_cases: Vec<(&str, &str)> = vec![
        ("/api/v1/nonexistent", "completely invalid path"),
        ("/api/v3/orders", "wrong API version"),
        (&extra_segment_path, "extra path segment after order UID"),
        ("/v1/orders", "missing /api prefix"),
        ("/api/v1/order", "typo - singular instead of plural"),
        (
            &wrong_nested_path,
            "wrong nested path - trades instead of orders",
        ),
        ("/api/v1/tokens", "nonexistent tokens endpoint"),
        ("/api/v2/orders", "v2 orders doesn't exist"),
    ];

    for (path, description) in missing_endpoint_cases {
        let response = client
            .get(format!("{API_HOST}{path}"))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404 for {description}: {path}"
        );
    }

    // Test malformed request bodies - Invalid JSON syntax
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
        StatusCode::UNPROCESSABLE_ENTITY,
        "Missing required fields should return 422"
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
        StatusCode::UNPROCESSABLE_ENTITY,
        "Wrong field types should return 422"
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
        StatusCode::UNPROCESSABLE_ENTITY,
        "Invalid enum value should return 422"
    );

    // Test error response formats
    // Deserialization errors return plain text with error description
    let response = client
        .post(format!("{API_HOST}/api/v1/orders"))
        .header("Content-Type", "application/json")
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body_text = response.text().await.unwrap();
    assert!(
        body_text.contains("deserialize")
            || body_text.contains("missing field")
            || body_text.contains("Failed to deserialize"),
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

    // /api/v1/orders/by_uids requires Json body
    let response = client
        .post(format!("{API_HOST}/api/v1/orders/by_uids"))
        .header("Content-Type", "application/json")
        .body("Not json")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Querying for more than ORDER_UID_LIMIT orders should fail
    let response = client
        .post(format!("{API_HOST}/api/v1/orders/by_uids"))
        .header("Content-Type", "application/json")
        .json(&json!(
            (0..ORDER_UID_LIMIT + 1)
                .map(|_| OrderUid::default())
                .collect::<Vec<_>>()
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // GET /restricted/api/v1/debug/simulation/{uid} error cases

    // Malformed UID → 400
    let response = client
        .get(format!(
            "{API_HOST}/restricted/api/v1/debug/simulation/bad_uid"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "malformed UID should return 400"
    );

    // Valid UID but order not found → 404
    let response = client
        .get(format!(
            "{API_HOST}/restricted/api/v1/debug/simulation/{VALID_ORDER_UID}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "unknown order UID should return 404"
    );
    let body: Error = response.json().await.unwrap();
    assert!(!body.error_type.is_empty());
    assert!(!body.description.is_empty());

    // Invalid block_number query param → 400
    let response = client
        .get(format!(
            "{API_HOST}/restricted/api/v1/debug/simulation/{VALID_ORDER_UID}?\
             block_number=notanumber"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "non-numeric block_number should return 400"
    );

    // POST /restricted/api/v1/debug/simulation error cases

    // Invalid JSON body → 400
    let response = client
        .post(format!("{API_HOST}/restricted/api/v1/debug/simulation"))
        .header("Content-Type", "application/json")
        .body("{invalid json}")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "invalid JSON should return 400"
    );

    // Missing required fields → 422
    let response = client
        .post(format!("{API_HOST}/restricted/api/v1/debug/simulation"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "missing required fields should return 422"
    );

    // Invalid field type (bad address) → 422
    let response = client
        .post(format!("{API_HOST}/restricted/api/v1/debug/simulation"))
        .json(&json!({
            "sellToken": "not-an-address",
            "buyToken": VALID_ADDRESS,
            "sellAmount": "1000000000000000000",
            "buyAmount": "1000000000000000000",
            "kind": "sell",
            "owner": VALID_ADDRESS,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid address field should return 422"
    );

    // Zero sellAmount (NonZeroU256 rejects zero at deserialization) → 422
    let response = client
        .post(format!("{API_HOST}/restricted/api/v1/debug/simulation"))
        .json(&json!({
            "sellToken": VALID_ADDRESS,
            "buyToken": VALID_ADDRESS,
            "sellAmount": "0",
            "buyAmount": "1000000000000000000",
            "kind": "sell",
            "owner": VALID_ADDRESS,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "zero sellAmount should return 422"
    );

    // Invalid kind enum value → 422
    let response = client
        .post(format!("{API_HOST}/restricted/api/v1/debug/simulation"))
        .json(&json!({
            "sellToken": VALID_ADDRESS,
            "buyToken": VALID_ADDRESS,
            "sellAmount": "1000000000000000000",
            "buyAmount": "1000000000000000000",
            "kind": "unknownKind",
            "owner": VALID_ADDRESS,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid kind enum should return 422"
    );

    // Invalid appData (non-JSON string triggers MalformedInput) → 400
    let bad_app_data = "not valid json";
    let response = client
        .post(format!("{API_HOST}/restricted/api/v1/debug/simulation"))
        .json(&json!({
            "sellToken": VALID_ADDRESS,
            "buyToken": VALID_ADDRESS,
            "sellAmount": "1000000000000000000",
            "buyAmount": "1000000000000000000",
            "kind": "sell",
            "owner": VALID_ADDRESS,
            "appData": bad_app_data,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "malformed appData should return 400"
    );
    let body: Error = response.json().await.unwrap();
    assert!(
        body.description.contains("app_data"),
        "error description should name the failing field. Got: {}",
        body.description
    );
    assert!(
        body.description.contains(bad_app_data),
        "error description should include the bad value. Got: {}",
        body.description
    );
}

#[tokio::test]
#[ignore]
async fn local_node_simulation_not_enabled() {
    run_test(simulation_not_enabled).await;
}

async fn simulation_not_enabled(web3: Web3) {
    let onchain = OnchainComponents::deploy(web3).await;
    let services = Services::new(&onchain).await;
    services
        .start_api(configs::orderbook::Configuration {
            order_simulation: None,
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "test_quoter",
                "http://localhost:11088/test_solver",
            )]),
            shared: SharedConfig {
                gas_estimators: vec![TestDefault::test_default()],
                ..Default::default()
            },
            ..configs::orderbook::Configuration::test_default()
        })
        .await;
    let client = services.client();

    // GET → 405 when simulation is not enabled
    let response = client
        .get(format!(
            "{API_HOST}/restricted/api/v1/debug/simulation/{VALID_ORDER_UID}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "GET simulation with disabled endpoint should return 405"
    );

    // POST → 405 when simulation is not enabled
    let response = client
        .post(format!("{API_HOST}/restricted/api/v1/debug/simulation"))
        .json(&json!({
            "sellToken": VALID_ADDRESS,
            "buyToken": VALID_ADDRESS,
            "sellAmount": "1000000000000000000",
            "buyAmount": "1000000000000000000",
            "kind": "sell",
            "owner": VALID_ADDRESS,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "POST simulation with disabled endpoint should return 405"
    );
}
