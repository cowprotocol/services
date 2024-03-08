//! Tests that concentrated liquidity pools (e.g. Uniswap v3) can be
//! (de)serialized.

use {
    crate::tests::{self, legacy, mock},
    serde_json::json,
};

#[tokio::test]
async fn test() {
    let legacy_solver = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: mock::http::RequestBody::Exact(json!({
            "amms": {
                "0x97b744df0b59d93a866304f97431d8efad29a08d": {
                    "address": "0x97b744df0b59d93a866304f97431d8efad29a08d",
                    "cost": {
                        "amount": "1650000000000000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "fee": "0.003",
                    "kind": "Concentrated",
                    "mandatory": false,
                    "pool": {
                        "gas_stats": {
                            "mean": "110000"
                        },
                        "state": {
                            "liquidity": "200000000000",
                            "liquidity_net": {
                                "-3": "-123",
                                "3": "432"
                            },
                            "sqrt_price": "1000000000",
                            "tick": "-1",
                        },
                        "tokens": [
                            {
                                "decimals": "18",
                                "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                            },
                            {
                                "decimals": "18",
                                "id": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                            }
                        ]
                    },
                },
            },
            "metadata": {
                "auction_id": 1,
                "environment": "Ethereum / Mainnet",
                "gas_price": 15000000000.0,
                "native_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "run_id": null
            },
            "orders": {},
            "tokens": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "decimals": 18,
                    "normalize_priority": 1,
                    "accepted_for_internalization": false,
                    "internal_buffer": null,
                    "alias": null,
                    "external_price": null,
                }
            }
        })),
        res: json!({
            "orders": {},
            "prices": {},
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
            "tokens": {},
            "orders": [],
            "liquidity": [
                {
                    "kind": "concentratedLiquidity",
                    "id": "0",
                    "address": "0x97b744df0b59d93A866304f97431D8EfAd29a08d",
                    "router": "0xe592427a0aece92de3edee1f18e0157c05861564",
                    "gasEstimate": "110000",
                    "tokens": [
                        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                        "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"
                    ],
                    "liquidity": "200000000000",
                    "tick": -1,
                    "sqrtPrice": "1000000000",
                    "liquidityNet": {
                        "-3": "-123",
                        "3": "432"
                    },
                    "fee": "0.003"
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
                "prices": {},
                "trades": [],
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
                "score": {
                    "kind": "riskAdjusted",
                    "successProbability": 1.0,
                }
            }]
        }),
    );
}
