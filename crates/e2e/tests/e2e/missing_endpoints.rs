//! Tests for missing/invalid endpoints to ensure 404 responses are preserved.

use {
    e2e::setup::{API_HOST, OnchainComponents, Services, run_test},
    number::units::EthUnit,
    reqwest::StatusCode,
    shared::ethrpc::Web3,
};

const VALID_ORDER_UID: &str = "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
const VALID_ADDRESS: &str = "0x0000000000000000000000000000000000000001";

#[tokio::test]
#[ignore]
async fn local_node_missing_endpoints() {
    run_test(missing_endpoints).await;
}

async fn missing_endpoints(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let client = services.client();

    let extra_segment_path = format!("/api/v1/orders/{VALID_ORDER_UID}/extra");
    let wrong_nested_path = format!("/api/v1/account/{VALID_ADDRESS}/trades");

    let test_cases: Vec<(&str, &str)> = vec![
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

    for (path, description) in test_cases {
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
}
