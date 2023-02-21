//! This test ensures that the Balancer solver properly handles cases where no
//! swap was found for the specified quoted order.

use {
    crate::tests::{self, balancer, mock},
    serde_json::json,
};

/// Tests that orders get marked as "mandatory" in `/quote` requests.
#[tokio::test]
async fn test() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: json!({
            "sellToken": "0x1111111111111111111111111111111111111111",
            "buyToken": "0x2222222222222222222222222222222222222222",
            "orderKind": "sell",
            "amount": "1000000000000000000",
            "gasPrice": "15000000000",
        }),
        res: json!({
            "tokenAddresses": [],
            "swaps": [],
            "swapAmount": "0",
            "swapAmountForSwaps": "0",
            "returnAmount": "0",
            "returnAmountFromSwaps": "0",
            "returnAmountConsideringFees": "0",
            "tokenIn": "",
            "tokenOut": "",
            "marketSp": "0",
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api)).await;

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
                    "reward": 0.,
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
