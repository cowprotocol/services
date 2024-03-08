//! This test verifies that the limit order's limit price is respected after
//! surplus fees are taken from the order.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn test() {
    let engine = tests::SolverEngine::new(
        "naive",
        tests::Config::String(r#"risk-parameters = [0,0,0,0]"#.to_owned()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "buyToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "sellAmount": "22397494",
                    "buyAmount": "18477932550000000",
                    "feeAmount": "1675785",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "limit",
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {
                            "balance": "36338096110368"
                        },
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                            "balance": "30072348537379906026018"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0x0000000000000000000000000000000000000000",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
                    "gasEstimate": "110000"
                },
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": []
        }),
    );
}
