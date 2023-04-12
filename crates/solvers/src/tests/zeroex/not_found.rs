//! This test ensures that the 0x solver properly handles cases where no swap
//! was found for the specified quoted order.

use {
    crate::tests::{self, mock, zeroex},
    serde_json::json,
};

/// Tests that orders get marked as "mandatory" in `/quote` requests.
#[tokio::test]
async fn test() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::Any,
        res: json!({
            "code": 100,
            "reason": "Validation Failed",
            "validationErrors": [
                {
                    "field": "buyAmount",
                    "code": 1004,
                    "reason": "INSUFFICIENT_ASSET_LIQUIDITY",
                    "description": "We are not able to fulfill an order for this token pair \
                                    at the requested amount due to a lack of liquidity",
                },
            ],
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("zeroex", zeroex::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": null,
            "tokens": {},
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0x1111111111111111111111111111111111111111",
                    "buyToken": "0x2222222222222222222222222222222222222222",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "1000000000000000000",
                    "feeAmount": "1000000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                },
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "prices": {},
            "trades": [],
            "interactions": [],
        }),
    );
}
