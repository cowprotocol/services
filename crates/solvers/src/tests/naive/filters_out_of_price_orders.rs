//! This test verifies that orders that are out of price get filtered out, but
//! a solution with the "reasonably" priced orders is produced.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn sell_orders_on_both_sides() {
    let engine = tests::SolverEngine::new("naive", tests::Config::None).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                // Unreasonable order a -> b
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "1000000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                // Reasonable order a -> b
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "1000000000000000000000",
                    "buyAmount": "1000000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                // Reasonable order a -> b
                {
                    "uid": "0x0303030303030303030303030303030303030303030303030303030303030303\
                              0303030303030303030303030303030303030303\
                              03030303",
                    "sellToken": "0x000000000000000000000000000000000000000b",
                    "buyToken": "0x000000000000000000000000000000000000000a",
                    "sellAmount": "1000000000000000000000",
                    "buyAmount": "1000000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                // Unreasonable order a -> b
                {
                    "uid": "0x0404040404040404040404040404040404040404040404040404040404040404\
                              0404040404040404040404040404040404040404\
                              04040404",
                    "sellToken": "0x000000000000000000000000000000000000000b",
                    "buyToken": "0x000000000000000000000000000000000000000a",
                    "sellAmount": "2000000000000000000",
                    "buyAmount": "1000000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x000000000000000000000000000000000000000a": {
                            "balance": "1000000000000000000000000"
                        },
                        "0x000000000000000000000000000000000000000b": {
                            "balance": "1000000000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xffffffffffffffffffffffffffffffffffffffff",
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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x000000000000000000000000000000000000000a": "1000000000000000000000000",
                    "0x000000000000000000000000000000000000000b": "1000000000000000000000000",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0303030303030303030303030303030303030303030303030303030303030303\
                                    0303030303030303030303030303030303030303\
                                    03030303",
                        "executedAmount": "1000000000000000000000",
                    },
                    {
                        "kind": "fulfillment",
                        "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                    0202020202020202020202020202020202020202\
                                    02020202",
                        "executedAmount": "1000000000000000000000",
                    },
                ],
                "interactions": [],
                "score": {
                    "kind": "riskAdjusted",
                    "successProbability": 0.5,
                },
                "gas": 259417,
            }]
        }),
    );
}
