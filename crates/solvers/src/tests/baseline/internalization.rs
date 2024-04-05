//! Simple test case that verifies that the baseline solver internalizes
//! eligible interactions using Settlement contract buffers.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn trusted_token() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::File("config/example.baseline.toml".into()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1412206645170290748",
                    "trusted": true
                },
                "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                    "decimals": 18,
                    "symbol": "COW",
                    "referencePrice": "53125132573502",
                    "availableBalance": "78402641384835564507389",
                    "trusted": true
                }
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
                    "sellAmount": "133700000000000000",
                    "buyAmount": "6000000000000000000000",
                    "feeAmount": "4200000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                            "balance": "3828187314911751990"
                        },
                        "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                            "balance": "179617892578796375604692"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0x97b744df0b59d93A866304f97431D8EfAd29a08d",
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
                    "gasEstimate": "110000"
                }
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "6043910341261930467761",
                    "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "133700000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "133700000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": true,
                        "id": "0",
                        "inputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "outputToken": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "inputAmount": "133700000000000000",
                        "outputAmount": "6043910341261930467761"
                    }
                ],
                "gas": 166391,
            }]
        }),
    );
}

#[tokio::test]
async fn untrusted_sell_token() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::File("config/example.baseline.toml".into()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1412206645170290748",
                    "trusted": false
                },
                "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                    "decimals": 18,
                    "symbol": "COW",
                    "referencePrice": "53125132573502",
                    "availableBalance": "78402641384835564507389",
                    "trusted": true
                }
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
                    "sellAmount": "133700000000000000",
                    "buyAmount": "6000000000000000000000",
                    "feeAmount": "4200000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                            "balance": "3828187314911751990"
                        },
                        "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                            "balance": "179617892578796375604692"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0x97b744df0b59d93A866304f97431D8EfAd29a08d",
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
                    "gasEstimate": "110000"
                }
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "6043910341261930467761",
                    "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "133700000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "133700000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "outputToken": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "inputAmount": "133700000000000000",
                        "outputAmount": "6043910341261930467761"
                    }
                ],
                "gas": 166391,
            }]
        }),
    );
}

#[tokio::test]
async fn insufficient_balance() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::File("config/example.baseline.toml".into()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1412206645170290748",
                    "trusted": true
                },
                "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                    "decimals": 18,
                    "symbol": "COW",
                    "referencePrice": "53125132573502",
                    "availableBalance": "0",
                    "trusted": true
                }
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
                    "sellAmount": "133700000000000000",
                    "buyAmount": "6000000000000000000000",
                    "feeAmount": "4200000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                            "balance": "3828187314911751990"
                        },
                        "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                            "balance": "179617892578796375604692"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0x97b744df0b59d93A866304f97431D8EfAd29a08d",
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
                    "gasEstimate": "110000"
                }
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "6043910341261930467761",
                    "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "133700000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "133700000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "outputToken": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "inputAmount": "133700000000000000",
                        "outputAmount": "6043910341261930467761"
                    }
                ],
                "gas": 166391,
            }]
        }),
    );
}
