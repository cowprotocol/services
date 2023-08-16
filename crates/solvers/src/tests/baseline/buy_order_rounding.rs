//! Simple test case that verifies that the baseline solver can settle a buy
//! orders, and deal with weird rounding behaviour.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn uniswap() {
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
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48": {
                    "decimals": 6,
                    "symbol": "USDC",
                    "referencePrice": "543222446200924874026413848",
                    "availableBalance": "556450389",
                    "trusted": true
                }
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                    "buyToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "sellAmount": "2000000000",
                    "buyAmount": "1000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [
                {
                    "kind": "constantproduct",
                    "tokens": {
                        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48": {
                            "balance": "30493445841295"
                        },
                        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                            "balance": "16551311935742077745684"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc",
                    "gasEstimate": "110000"
                }
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    // Note that the interaction executes slightly more than the buy order's
    // amount. This is inevitable because of rounding - if we sold 1 less wei
    // of the input token, we would not be able to buy enough to cover the buy
    // order, the difference stays in the settlement contract.
    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1848013595",
                    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "1000000000000000000"
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
                        "inputToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                        "outputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "inputAmount": "1848013595",
                        "outputAmount": "1000000000428620302"
                    }
                ]
            }]
        }),
    );
}

#[tokio::test]
async fn balancer() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = ["0x9c58bacc331c9aa871afd802db6379a98e80cedb"]
                max-hops = 1
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
                "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d": {
                    "decimals": 18,
                    "symbol": "wxDAI",
                    "referencePrice": null,
                    "availableBalance": "0",
                    "trusted": true
                },
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
                    "uid": "0x0000000000000000000000000000000000000000000000000000000000000000\
                              0000000000000000000000000000000000000000\
                              00000000",
                    "sellToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                    "buyToken": "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d",
                    "sellAmount": "22300745198530623141535718272648361505980416",
                    "buyAmount": "1000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [
                // A xCOW -> xGNO -> wxDAI path with a good price.
                {
                    "kind": "constantproduct",
                    "tokens": {
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "9661963829146095661"
                        },
                        "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d": {
                            "balance": "1070533209145548137343"
                        }
                    },
                    "fee": "0.0025",
                    "id": "0",
                    "address": "0xd7b118271b1b7d26c9e044fc927ca31dccb22a5a",
                    "gasEstimate": "90171"
                },
                {
                    "kind": "weightedproduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "1963528800698237927834721",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "1152796145430714835825",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "1",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd99",
                    "gasEstimate": "88892"
                },
                // A fake xCOW -> wxDAI path with a BAD price.
                {
                    "kind": "constantproduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "1000000000000000000000000000"
                        },
                        "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d": {
                            "balance": "1000000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "2",
                    "address": "0x9090909090909090909090909090909090909090",
                    "gasEstimate": "90171"
                },
            ],
            "effectiveGasPrice": "1000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    // Note that the interaction executes slightly more than the buy order's
    // amount. This is inevitable because of rounding.
    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x177127622c4a00f3d409b75571e12cb3c8973d3c": "1000000000000000000",
                    "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d": "15503270361046566989"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0000000000000000000000000000000000000000000000000000000000000000\
                                    0000000000000000000000000000000000000000\
                                    00000000",
                        "executedAmount": "1000000000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "1",
                        "inputToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                        "outputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "inputAmount": "15503270361046566989",
                        "outputAmount": "9056454904358278"
                    },
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "outputToken": "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d",
                        "inputAmount": "9056454904358278",
                        "outputAmount": "1000000000000082826"
                    },
                ]
            }]
        }),
    );
}

#[tokio::test]
async fn same_path() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = ["0x9c58bacc331c9aa871afd802db6379a98e80cedb"]
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
                    "sellToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                    "buyToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                    "sellAmount": "16000000000000000000",
                    "buyAmount": "9056454904357528",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [
                {
                    "kind": "weightedproduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "1963528800698237927834721",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "1152796145430714835825",
                            "scalingFactor": "1000000000000000000",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "0",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd99",
                    "gasEstimate": "0"
                },
                {
                    "kind": "constantproduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "1000000000000000000000000000"
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "585921934616708391829053"
                        }
                    },
                    "fee": "0.003",
                    "id": "1",
                    "address": "0x9090909090909090909090909090909090909090",
                    "gasEstimate": "0"
                },
            ],
            "effectiveGasPrice": "1000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    // Lets get down to the math!
    // --------------------------
    //
    // Balancer V2 weighted pools are unstable - this means that when computing
    // `get_amount_out(TOK0, (get_amount_in(TOK1, (a, TOK0)), TOK1)) < a`,
    // meaning that we actually need to sell a bit more than computed in order
    // to cover the buy order. Specifically, in this example:
    // - the computed input amount is `15503270361045187239 xCOW`
    // - the corresponding output amount is `9056454904357125 xGNO` (`403` less than
    //   what is actually needed)
    // - the optimal input amount is `15503270361046529181 xCOW`, such that this
    //   amount -1 would result in an output amount that is not enough to cover the
    //   order
    //
    // Interestingly, in the same path, we have an constant product pool (i.e.
    // Uniswap-like pool) L1 which if used for a solution would result in
    // selling an amount that is higher than the computed input amount for the
    // Balancer pool, but lower than its optimal input amount.
    //
    // This tests asserts that we use L1 in the solution.
    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x177127622c4a00f3d409b75571e12cb3c8973d3c": "9056454904357528",
                    "0x9c58bacc331c9aa871afd802db6379a98e80cedb": "15503270361045187242"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "9056454904357528"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "1",
                        "inputToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                        "outputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "inputAmount": "15503270361045187242",
                        "outputAmount": "9056454904357528"
                    },
                ]
            }]
        }),
    );
}
