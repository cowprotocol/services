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
                native-token-price-estimation-amount = "100000000000000000"
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
                    "feePolicies": [],
                }
            ],
            "liquidity": [
                {
                    "kind": "weightedProduct",
                    "tokens": {
                        "0x6810e776880c02933d47db1b9fc05908e5386b96": {
                            "balance": "11260752191375725565253",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        },
                        "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": {
                            "balance": "18764168403990393422000071",
                            "scalingFactor": "1",
                            "weight": "0.5",
                        }
                    },
                    "fee": "0.005",
                    "id": "0",
                    "address": "0x92762b42a06dcdddc5b7362cfb01e631c4d44b40",
                    "balancerPoolId": "0x5c78d05b8ecf97507d1cf70646082c54faa4da950000000000000000000005ca",
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
                ],
                "gas": 206391,
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
                native-token-price-estimation-amount = "1000000000000000000"
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
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x9c58bacc331c9aa871afd802db6379a98e80cedb": {
                            "balance": "20000000000000000000000",
                        },
                        "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d": { // native token on gnosis chain
                            "balance": "1000000000000000000000",
                        }
                    },
                    "fee": "0.0025",
                    "id": "1",
                    "address": "0x21d4c792ea7e38e0d0819c2011a2b1cb7252bd98",
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
                    "gasEstimate": "88892",
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
                ],
                "gas": 206391,
            }]
        }),
    );
}

#[tokio::test]
async fn stable() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "1"
                base-tokens = []
                max-hops = 0
                max-partial-attempts = 1
                native-token-price-estimation-amount = "100000000000000000"
            "#
            .to_owned(),
        ),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0x6b175474e89094c44da98b954eedeac495271d0f": {
                    "decimals": 18,
                    "symbol": "DAI",
                    "referencePrice": "597423824203645",
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {
                    "decimals": 6,
                    "symbol": "USDC",
                    "referencePrice": "597647838715990684620292096",
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
            },
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x6b175474e89094c44da98b954eedeac495271d0f",
                    "buyToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "sellAmount": "10000000000000000000",
                    "buyAmount": "9500000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x6b175474e89094c44da98b954eedeac495271d0f",
                    "buyToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "sellAmount": "10500000000000000000",
                    "buyAmount": "10000000",
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
                        "0x6b175474e89094c44da98b954eedeac495271d0f": {
                            "balance": "505781036390938593206504",
                            "scalingFactor": "1",
                        },
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {
                            "balance": "554894862074",
                            "scalingFactor": "1000000000000",
                        },
                        "0xdac17f958d2ee523a2206206994597c13d831ec7": {
                            "balance": "1585576741011",
                            "scalingFactor": "1000000000000",
                        },
                    },
                    "fee": "0.0001",
                    "amplificationParameter": "5000.0",
                    "id": "0",
                    "address": "0x06df3b2bbb68adc8b0e302443692037ed9f91b42",
                    "balancerPoolId": "0x5c78d05b8ecf97507d1cf70646082c54faa4da950000000000000000000005ca",
                    "gasEstimate": "183520",
                },
            ],
            "effectiveGasPrice": "1000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [
                {
                    "id": 0,
                    "prices": {
                        "0x6b175474e89094c44da98b954eedeac495271d0f": "9999475",
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "10000000000000000000"
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
                            "inputToken": "0x6b175474e89094c44da98b954eedeac495271d0f",
                            "outputToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                            "inputAmount": "10000000000000000000",
                            "outputAmount": "9999475"
                        },
                    ],
                    "gas":  289911,
                },
                {
                    "id": 1,
                    "prices": {
                        "0x6b175474e89094c44da98b954eedeac495271d0f": "10000000",
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "10000524328839166557"
                    },
                    "trades": [
                        {
                            "kind": "fulfillment",
                            "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                        0202020202020202020202020202020202020202\
                                        02020202",
                            "executedAmount": "10000000"
                        }
                    ],
                    "interactions": [
                        {
                            "kind": "liquidity",
                            "internalize": false,
                            "id": "0",
                            "inputToken": "0x6b175474e89094c44da98b954eedeac495271d0f",
                            "outputToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                            "inputAmount": "10000524328839166557",
                            "outputAmount": "10000000"
                        },
                    ],
                    "gas":  289911,
                },
            ]
        }),
    );
}

#[tokio::test]
async fn composable_stable_v4() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::String(
            r#"
                chain-id = "100"
                base-tokens = []
                max-hops = 0
                max-partial-attempts = 1
                native-token-price-estimation-amount = "1000000000000000000"
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
                    "sellAmount": "10000000000000000000",
                    "buyAmount": "9500000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
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

    assert_eq!(
        solution,
        json!({
            "solutions": [
                {
                    "id": 0,
                    "prices": {
                        "0x4b1e2c2762667331bc91648052f646d1b0d35984": "10029862202766050434",
                        "0xcb444e90d8198415266c6a2724b7900fb12fc56e": "10000000000000000000"
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
                            "inputAmount": "10000000000000000000",
                            "outputAmount": "10029862202766050434"
                        },
                    ],
                    "gas": 289911,
                },
            ]
        }),
    );
}
