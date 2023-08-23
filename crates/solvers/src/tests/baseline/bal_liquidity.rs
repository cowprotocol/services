//! Test cases to verify baseline computation of Balancer V2 liquidity.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn weighted() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "1"
                base-tokens = []
                max-hops = 0
                max-partial-attempts = 1
            "#
            .to_owned(),
        ),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0x6810e776880c02933d47db1b9fc05908e5386b96": {
                    "decimals": 18,
                    "symbol": "GNO",
                    "referencePrice": "59970737022467696",
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": {
                    "decimals": 18,
                    "symbol": "COW",
                    "referencePrice": "35756662383952",
                    "availableBalance": "0",
                    "trusted": true
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                    "buyToken": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "1",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [
                {
                    "kind": "weightedproduct",
                    "tokens": {
                        "0x6810e776880c02933d47db1b9fc05908e5386b96": {
                            "balance": "11260752191375725565253",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        },
                        "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": {
                            "balance": "18764168403990393422000071",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "0",
                    "address": "0x92762b42a06dcdddc5b7362cfb01e631c4d44b40",
                    "gasEstimate": "88892",
                    "version": "v0",
                },
            ],
            "effectiveGasPrice": "1000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x6810e776880c02933d47db1b9fc05908e5386b96": "1657855325872947866705",
                    "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "1000000000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                        "outputToken": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "inputAmount": "1000000000000000000",
                        "outputAmount": "1657855325872947866705"
                    },
                ]
            }]
        }),
    );
}

#[tokio::test]
async fn weighted_v3plus() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = []
                max-hops = 0
                max-partial-attempts = 1
            "#
            .to_owned(),
        ),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                    "decimals": 18,
                    "symbol": "xCOW",
                    "referencePrice": null,
                    "availableBalance": "0",
                    "trusted": true
                },
                "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                    "decimals": 18,
                    "symbol": "xGNO",
                    "referencePrice": null,
                    "availableBalance": "0",
                    "trusted": true
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                    "buyToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "1",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [
                {
                    "kind": "weightedproduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "5089632258314443812936111",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "3043530764763263654069",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "0",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd99",
                    "gasEstimate": "88892",
                    "version": "v3plus",
                },
            ],
            "effectiveGasPrice": "1000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x177127622c4a00f3d409b75571e12cb3c8973d3c": "1000000000000000000",
                    "0x9c58bacc331c9aa871afd802db6379a98e80cedb": "1663373703594405548696"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "outputToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                        "inputAmount": "1000000000000000000",
                        "outputAmount": "1663373703594405548696"
                    },
                ]
            }]
        }),
    );
}
