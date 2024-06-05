//! Test cases to verify that baseline can quote using limit orders.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn sell_order() {
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
                    "fullSellAmount": "1000000000000000000",
                    "buyAmount": "1",
                    "fullBuyAmount": "1",
                    "feePolicies": [],
                    "validTo": 0,
                    "kind": "sell",
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "partiallyFillable": false,
                    "preInteractions": [],
                    "postInteractions": [],
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "class": "limit",
                    "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                    "signingScheme": "presign",
                    "signature": "0x",
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
            "deadline": "2106-01-01T00:00:00.000Z",
            "surplusCapturingJitOrderOwners": []
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x177127622c4a00f3d409b75571e12cb3c8973d3c": "995857692278744911",
                    "0x9c58bacc331c9aa871afd802db6379a98e80cedb": "1656483497858673768805"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "995857692278744911",
                        "fee": "4142307721255089"
                    }
                ],
                "preInteractions": [],
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
                "postInteractions": [],
                "gas": 206391,
            }]
        }),
    );
}

#[tokio::test]
async fn buy_order() {
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
                    "sellAmount": "1000000000000000000000000",
                    "fullSellAmount": "1000000000000000000000000",
                    "buyAmount": "1000000000000000000",
                    "fullBuyAmount": "1000000000000000000",
                    "feePolicies": [],
                    "validTo": 0,
                    "kind": "buy",
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "partiallyFillable": false,
                    "preInteractions": [],
                    "postInteractions": [],
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "class": "limit",
                    "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                    "signingScheme": "presign",
                    "signature": "0x",
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
            "deadline": "2106-01-01T00:00:00.000Z",
            "surplusCapturingJitOrderOwners": []
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x177127622c4a00f3d409b75571e12cb3c8973d3c": "600991453799057",
                    "0x9c58bacc331c9aa871afd802db6379a98e80cedb": "1000000000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000",
                        "fee": "4142307721255089"
                    }
                ],
                "preInteractions": [],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                        "outputToken": "0x177127622c4a00f3d409b75571e12cb3c8973d3c",
                        "inputAmount": "600991453799057",
                        "outputAmount": "1000000000004392195"
                    },
                ],
                "postInteractions": [],
                "gas": 206391,
            }]
        }),
    );
}
