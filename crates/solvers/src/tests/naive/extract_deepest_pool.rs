//! Test that demonstrates that the Naive solver will use the **deepest** pool
//! for solving when multiple different UniswapV2-like pools exist for a
//! specific token pair.
//!
//! The rationale behind this choise is that the deepest pools are typically the
//! most representative of the actual token price, and in general give better
//! prices for larger orders (in theory, for smaller orders it is possible for
//! the shallow pool to offer better prices, but this should be in exceptional
//! cases, and not worth considering in the solver in order to keep things
//! simple).

use {crate::tests, serde_json::json};

#[tokio::test]
async fn test() {
    let engine = tests::SolverEngine::new("naive", tests::Config::None).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0x0101010101010101010101010101010101010101",
                    "buyToken": "0x0202020202020202020202020202020202020202",
                    "sellAmount": "100",
                    "fullSellAmount": "1000000000000000000",
                    "buyAmount": "1",
                    "fullBuyAmount": "1000000000000000000000",
                    "feePolicies": [],
                    "validTo": 0,
                    "kind": "sell",
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "partiallyFillable": false,
                    "preInteractions": [],
                    "postInteractions": [],
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "class": "market",
                    "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                    "signingScheme": "presign",
                    "signature": "0x",
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x0101010101010101010101010101010101010101": {
                            "balance": "100"
                        },
                        "0x0202020202020202020202020202020202020202": {
                            "balance": "100"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0x2222222222222222222222222222222222222222",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
                    "gasEstimate": "0"
                },
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x0101010101010101010101010101010101010101": {
                            "balance": "10000000"
                        },
                        "0x0202020202020202020202020202020202020202": {
                            "balance": "10000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "1",
                    "address": "0x1111111111111111111111111111111111111111",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
                    "gasEstimate": "0"
                },
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x0303030303030303030303030303030303030303": {
                            "balance": "10000000000000000"
                        },
                        "0x0404040404040404040404040404040404040404": {
                            "balance": "10000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "2",
                    "address": "0x3333333333333333333333333333333333333333",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
                    "gasEstimate": "0"
                },
            ],
            "effectiveGasPrice": "0",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x0101010101010101010101010101010101010101": "99",
                    "0x0202020202020202020202020202020202020202": "100",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "100",
                    },
                ],
                "preInteractions": [],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "1",
                        "inputToken": "0x0101010101010101010101010101010101010101",
                        "outputToken": "0x0202020202020202020202020202020202020202",
                        "inputAmount": "100",
                        "outputAmount": "99"
                    },
                ],
                "postInteractions": [],
                "gas": 94391,
            }]
        }),
    );
}
