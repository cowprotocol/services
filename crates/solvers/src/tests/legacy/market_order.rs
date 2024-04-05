//! Simple test case that verifies the solver can handle market orders.

use {
    crate::tests::{self, legacy, mock},
    serde_json::json,
};

/// Tests that orders get marked as "mandatory" in `/quote` requests and that
/// the HTTP query does not contain the `auction_id` parameter.
#[tokio::test]
async fn quote() {
    let legacy_solver = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::glob(
            "solve\
             [?]instance_name=*_Mainnet_1_1\
               &time_limit=*\
               &max_nr_exec_orders=100\
               &use_internal_buffers=true\
               &auction_id=1\
               &request_id=0"
        ),
        req: mock::http::RequestBody::Exact(json!({
            "amms": {
                "0x97b744df0b59d93a866304f97431d8efad29a08d": {
                    "address": "0x97b744df0b59d93a866304f97431d8efad29a08d",
                    "cost": {
                        "amount": "1650000000000000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "fee": "0.003",
                    "kind": "ConstantProduct",
                    "mandatory": false,
                    "reserves": {
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "3828187314911751990",
                        "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "179617892578796375604692"
                    }
                }
            },
            "metadata": {
                "auction_id": 1,
                "environment": "Ethereum / Mainnet",
                "gas_price": 15000000000.0,
                "native_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "run_id": null
            },
            "orders": {
                "0": {
                    "allow_partial_fill": false,
                    "buy_amount": "6000000000000000000000",
                    "buy_token": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                    "cost": {
                        "amount": "994725000000000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "fee": {
                        "amount": "4200000000000000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "has_atomic_execution": false,
                    "id": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a",
                    "is_liquidity_order": false,
                    "is_mature": true,
                    "is_sell_order": true,
                    "mandatory": false,
                    "reward": 0.,
                    "sell_amount": "133700000000000000",
                    "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                }
            },
            "tokens": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "accepted_for_internalization": true,
                    "alias": "WETH",
                    "decimals": 18,
                    "external_price": 1.0,
                    "internal_buffer": "1412206645170290748",
                    "normalize_priority": 1
                },
                "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": {
                    "accepted_for_internalization": true,
                    "alias": "COW",
                    "decimals": null,
                    "external_price": 0.000053125132573502,
                    "internal_buffer": "740264138483556450389",
                    "normalize_priority": 0
                }
            }
        })),
        res: json!({
            "orders": {
                "0": {
                    "exec_sell_amount": "133700000000000000",
                    "exec_buy_amount": "6000000000000000000000",
                    "exec_fee_amount": "6900000000000000"
                }
            },
            "prices": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "6043910341261930467761",
                "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "133700000000000000"
            },
            "amms": {
                "0x97b744df0b59d93a866304f97431d8efad29a08d": {
                    "execution": [{
                        "sell_token": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                        "buy_token": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
                        "exec_sell_amount": "133700000000000000",
                        "exec_buy_amount": "6043910341261930467761",
                        "exec_plan": {
                            "sequence": 0,
                            "position": 1,
                            "internal": false,
                        }
                    }],
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("legacy", legacy::config(&legacy_solver.address)).await;

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
                    "decimals": null,
                    "symbol": "COW",
                    "referencePrice": "53125132573502",
                    "availableBalance": "740264138483556450389",
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
                        "inputToken": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "outputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "inputAmount": "6043910341261930467761",
                        "outputAmount": "133700000000000000",
                    }
                ],
            }]
        }),
    );
}

#[tokio::test]
async fn solve() {
    let legacy_solver = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::glob(
            "solve\
             [?]instance_name=*_Mainnet_1_1234\
               &time_limit=*\
               &max_nr_exec_orders=100\
               &use_internal_buffers=true\
               &auction_id=1234\
               &request_id=0",
        ),
        req: mock::http::RequestBody::Exact(json!({
            "amms": {
                "0x97b744df0b59d93a866304f97431d8efad29a08d": {
                    "address": "0x97b744df0b59d93a866304f97431d8efad29a08d",
                    "cost": {
                        "amount": "1650000000000000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "fee": "0.003",
                    "kind": "ConstantProduct",
                    "mandatory": false,
                    "reserves": {
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "3828187314911751990",
                        "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "179617892578796375604692"
                    }
                }
            },
            "metadata": {
                "auction_id": 1234,
                "environment": "Ethereum / Mainnet",
                "gas_price": 15000000000.0,
                "native_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "run_id": null
            },
            "orders": {
                "0": {
                    "allow_partial_fill": false,
                    "buy_amount": "6000000000000000000000",
                    "buy_token": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                    "cost": {
                        "amount": "994725000000000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "fee": {
                        "amount": "4200000000000000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "has_atomic_execution": false,
                    "id": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a",
                    "is_liquidity_order": false,
                    "is_mature": true,
                    "is_sell_order": true,
                    "mandatory": false,
                    "reward": 0.,
                    "sell_amount": "133700000000000000",
                    "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                }
            },
            "tokens": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "accepted_for_internalization": true,
                    "alias": "WETH",
                    "decimals": 18,
                    "external_price": 1.0,
                    "internal_buffer": "1412206645170290748",
                    "normalize_priority": 1
                },
                "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": {
                    "accepted_for_internalization": true,
                    "alias": "COW",
                    "decimals": null,
                    "external_price": 0.000053125132573502,
                    "internal_buffer": "740264138483556450389",
                    "normalize_priority": 0
                }
            }
        })),
        res: json!({
            "orders": {
                "0": {
                    "exec_sell_amount": "133700000000000000",
                    "exec_buy_amount": "6000000000000000000000",
                    "exec_fee_amount": "6900000000000000",
                }
            },
            "prices": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "6043910341261930467761",
                "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": "133700000000000000"
            },
            "amms": {
                "0x97b744df0b59d93a866304f97431d8efad29a08d": {
                    "execution": [{
                        "sell_token": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                        "buy_token": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
                        "exec_sell_amount": "133700000000000000",
                        "exec_buy_amount": "6043910341261930467761",
                        "exec_plan": {
                            "sequence": 0,
                            "position": 1,
                            "internal": false,
                        }
                    }],
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("legacy", legacy::config(&legacy_solver.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1234",
            "tokens": {
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1412206645170290748",
                    "trusted": true
                },
                "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                    "decimals": null,
                    "symbol": "COW",
                    "referencePrice": "53125132573502",
                    "availableBalance": "740264138483556450389",
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
                        "inputToken": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "outputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "inputAmount": "6043910341261930467761",
                        "outputAmount": "133700000000000000",
                    }
                ],
            }]
        }),
    );
}
