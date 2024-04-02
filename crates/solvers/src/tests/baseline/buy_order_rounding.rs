//! Simple test cases that verify that the baseline solver can settle a buy
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
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
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
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
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
                        "internalize": true,
                        "id": "0",
                        "inputToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                        "outputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "inputAmount": "1848013595",
                        "outputAmount": "1000000000428620302"
                    }
                ],
                "gas": 166391,
            }]
        }),
    );
}

#[tokio::test]
async fn balancer_weighted() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = ["0x9c58bacc331c9aa871afd802db6379a98e80cedb"]
                max-hops = 1
                max-partial-attempts = 1
                risk-parameters = [0,0,0,0]
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
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                // A xCOW -> xGNO -> wxDAI path with a good price.
                {
                    "kind": "constantProduct",
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
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
                    "gasEstimate": "90171"
                },
                {
                    "kind": "weightedProduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "1963528800698237927834721",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "1152796145430714835825",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "1",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd99",
                    "balancerPoolId": "0x5c78d05b8ecf97507d1cf70646082c54faa4da950000000000000000000005ca",
                    "gasEstimate": "88892",
                    "version": "v0"
                },
                // A fake xCOW -> wxDAI path with a BAD price.
                {
                    "kind": "constantProduct",
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
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
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
                    "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d": "15503270361052085989"
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
                        "inputAmount": "15503270361052085989",
                        "outputAmount": "9056454904360584"
                    },
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "outputToken": "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d",
                        "inputAmount": "9056454904360584",
                        "outputAmount": "1000000000000337213"
                    },
                ],
                "gas": 266391,
            }]
        }),
    );
}

#[tokio::test]
async fn balancer_weighted_v3plus() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = []
                max-hops = 0
                max-partial-attempts = 1
                risk-parameters = [0,0,0,0]
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
                    "sellAmount": "100000000000000000000000",
                    "buyAmount": "1000000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "weightedProduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "18764168403990393422000071",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "11260752191375725565253",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "0",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd99",
                    "balancerPoolId": "0x5c78d05b8ecf97507d1cf70646082c54faa4da950000000000000000000005ca",
                    "gasEstimate": "88892",
                    "version": "v3Plus",
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
                    "0x177127622c4a00f3d409b75571e12cb3c8973d3c": "603167793526702182",
                    "0x9c58bacc331c9aa871afd802db6379a98e80cedb": "1000000000000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "outputToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                        "inputAmount": "603167793526702182",
                        "outputAmount": "1000000000000001964333"
                    },
                ],
                "gas": 206391,
            }]
        }),
    );
}

#[tokio::test]
async fn distant_convergence() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = []
                max-hops = 0
                max-partial-attempts = 1
                risk-parameters = [0,0,0,0]
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
                    "sellAmount": "100000000000000000000000",
                    "buyAmount": "999999999999999843119",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "weightedProduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "5089632258314443812936111",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "3043530764763263654069",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "0",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd99",
                    "balancerPoolId": "0x5c78d05b8ecf97507d1cf70646082c54faa4da950000000000000000000005ca",
                    "gasEstimate": "88892",
                    "version": "v3Plus",
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
                    "0x177127622c4a00f3d409b75571e12cb3c8973d3c": "601109440402472000",
                    "0x9c58bacc331c9aa871afd802db6379a98e80cedb": "999999999999999843119"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "999999999999999843119"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "outputToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                        "inputAmount": "601109440402472000",
                        "outputAmount": "1000000000000015112015"
                    },
                ],
                "gas": 206391,
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
                risk-parameters = [0,0,0,0]
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
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "weightedProduct",
                    "tokens": {
                        "0x177127622c4a00f3d409b75571e12cb3c8973d3c": {
                            "balance": "1963528800698237927834721",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        },
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "1152796145430714835825",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "0",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd99",
                    "balancerPoolId": "0x5c78d05b8ecf97507d1cf70646082c54faa4da950000000000000000000005ca",
                    "gasEstimate": "0",
                    "version": "v0",
                },
                {
                    "kind": "constantProduct",
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
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
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
                ],
                "gas": 166391,
            }]
        }),
    );
}

#[tokio::test]
async fn balancer_stable() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = []
                max-hops = 0
                max-partial-attempts = 1
                risk-parameters = [0,0,0,0]
            "#
            .to_owned(),
        ),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0x4b1e2c2762667331bc91648052f646d1b0d35984": {
                    "decimals": 18,
                    "symbol": "agEUR",
                    "referencePrice": "1090118822951692177",
                    "availableBalance": "0",
                    "trusted": false
                },
                "0x5c78d05b8ecf97507d1cf70646082c54faa4da95": {
                    "decimals": 18,
                    "symbol": "bb-agEUR-EURe",
                    "referencePrice": "10915976478387159906",
                    "availableBalance": "0",
                    "trusted": false
                },
                "0xcb444e90d8198415266c6a2724b7900fb12fc56e": {
                    "decimals": 18,
                    "symbol": "EURe",
                    "referencePrice": "10917431192660550458",
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d": {
                    "decimals": 18,
                    "symbol": "wxDAI",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "0",
                    "trusted": true
                },
            },
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x4b1e2c2762667331bc91648052f646d1b0d35984",
                    "buyToken": "0xcb444e90d8198415266c6a2724b7900fb12fc56e",
                    "sellAmount": "10500000000000000000",
                    "buyAmount": "10000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "stable",
                    "tokens": {
                        "0x4b1e2c2762667331bc91648052f646d1b0d35984": {
                            "balance": "126041615528606990697699",
                            "scalingFactor": "1",
                        },
                        "0x5c78d05b8ecf97507d1cf70646082c54faa4da95": {
                            "balance": "2596148429267369423681023550322451",
                            "scalingFactor": "1",
                        },
                        "0xcb444e90d8198415266c6a2724b7900fb12fc56e": {
                            "balance": "170162457652825667152980",
                            "scalingFactor": "1",
                        },
                    },
                    "fee": "0.0001",
                    "amplificationParameter": "100.0",
                    "id": "0",
                    "address": "0x5c78d05b8ecf97507d1cf70646082c54faa4da95",
                    "balancerPoolId": "0x5c78d05b8ecf97507d1cf70646082c54faa4da950000000000000000000005ca",
                    "gasEstimate": "183520",
                },
            ],
            "effectiveGasPrice": "1000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    // Here:
    //
    // ```
    // get_amount_in(1.0) = 9.970226684231795303
    // get_amount_out(9.970226684231795303) = 9.999999999999999999
    // get_amount_out(9.970226684231795304) = 1.0
    // ```
    assert_eq!(
        solution,
        json!({
            "solutions": [
                {
                    "id": 0,
                    "prices": {
                        "0x4b1e2c2762667331bc91648052f646d1b0d35984": "10000000000000000000",
                        "0xcb444e90d8198415266c6a2724b7900fb12fc56e": "9970226684231795304"
                    },
                    "trades": [
                        {
                            "kind": "fulfillment",
                            "order": "0x0101010101010101010101010101010101010101010101010101010101010101\
                                        0101010101010101010101010101010101010101\
                                        01010101",
                            "executedAmount": "10000000000000000000"
                        }
                    ],
                    "interactions": [
                        {
                            "kind": "liquidity",
                            "internalize": false,
                            "id": "0",
                            "inputToken": "0x4b1e2c2762667331bc91648052f646d1b0d35984",
                            "outputToken": "0xcb444e90d8198415266c6a2724b7900fb12fc56e",
                            "inputAmount": "9970226684231795304",
                            "outputAmount": "10000000000000000000"
                        },
                    ],
                    "gas": 289911,
                },
            ]
        }),
    );
}
