//! Tests that dex solvers can make progress on partially fillable orders across
//! multiple requests. We are using the balancer API here because that's the
//! easies to mock.

use {
    crate::tests::{self, balancer, mock},
    serde_json::json,
};

#[tokio::test]
async fn test() {
    let inner_request = |amount| {
        json!({
            "sellToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "buyToken": "0xba100000625a3754423978a60c9317c58a424e3d",
            "orderKind": "sell",
            "amount": amount,
            "gasPrice": "15000000000",
        })
    };

    // response returned when no route could be found
    let inner_response_error = json!({
        "tokenAddresses": [],
        "swaps": [],
        "swapAmount": "0",
        "swapAmountForSwaps": "0",
        "returnAmount": "0",
        "returnAmountFromSwaps": "0",
        "returnAmountConsideringFees": "0",
        "tokenIn": "0x0000000000000000000000000000000000000000",
        "tokenOut": "0x0000000000000000000000000000000000000000",
        "marketSp": "0",
    });

    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: inner_request("16000000000000000000"),
        res: inner_response_error.clone(),
    },
    mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: inner_request("8000000000000000000"),
        res: inner_response_error.clone(),
    },
    mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: inner_request("4000000000000000000"),
        res: inner_response_error.clone(),
    },
    mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: inner_request("2000000000000000000"),
        res: inner_response_error.clone(),
    },
    mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: inner_request("1000000000000000000"),
        res: json!({
            "tokenAddresses": [
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "0xba100000625a3754423978a60c9317c58a424e3d"
            ],
            "swaps": [
                {
                    "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
                    "assetInIndex": 0,
                    "assetOutIndex": 1,
                    "amount": "1000000000000000000",
                    "userData": "0x",
                    "returnAmount": "227598784442065388110"
                }
            ],
            "swapAmount": "1000000000000000000",
            "swapAmountForSwaps": "1000000000000000000",
            "returnAmount": "227598784442065388110",
            "returnAmountFromSwaps": "227598784442065388110",
            "returnAmountConsideringFees": "227307710853355710706",
            "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
            "marketSp": "0.004393607339632106",
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api)).await;

    let auction = json!({
        "id": null,
        "tokens": {
            "0xba100000625a3754423978a60c9317c58a424e3D": {
                "decimals": 18,
                "symbol": "BAL",
                "referencePrice": "4327903683155778",
                "availableBalance": "1583034704488033979459",
                "trusted": true
            },
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                "decimals": 18,
                "symbol": "WETH",
                "referencePrice": "1000000000000000000",
                "availableBalance": "482725140468789680",
                "trusted": true
            },
        },
        "orders": [
            {
                "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                          2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                          2a2a2a2a",
                "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                "buyToken": "0xba100000625a3754423978a60c9317c58a424e3D",
                "sellAmount": "16000000000000000000",
                "buyAmount": "3641580551073046209760",
                // Let's just assume 0 fee to not further complicate the math.
                "feeAmount": "0",
                "kind": "sell",
                "partiallyFillable": true,
                "class": "market",
                "reward": 0.
            }
        ],
        "liquidity": [],
        "effectiveGasPrice": "15000000000",
        "deadline": "2106-01-01T00:00:00.000Z"
    });

    for _ in 0..4 {
        let solution = engine.solve(auction.clone()).await;

        // No solution could be found so we'll try with a lower fill amount next time.
        assert_eq!(
            solution,
            json!({
                "prices": {},
                "trades": [],
                "interactions": [],
            }),
        );
    }

    let solution = engine.solve(auction.clone()).await;

    assert_eq!(
        solution,
        json!({
            "interactions": [
                {
                    "allowances": [
                        {
                            "amount": "1000000000000000000",
                            "spender": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                            "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                        }
                    ],
                    "calldata": "0x945bcec90000000000000000000000000000000000000000000\
                        00000000000000000000000000000000000000000000000000000000000000\
                        00000000000000000000120000000000000000000000000000000000000000\
                        00000000000000000000002200000000000000000000000009008d19f58aab\
                        d9ed0d60971565aa8510560ab4100000000000000000000000000000000000\
                        000000000000000000000000000000000000000000000000000009008d19f5\
                        8aabd9ed0d60971565aa8510560ab410000000000000000000000000000000\
                        00000000000000000000000000000000000000000000000000000000000000\
                        00000000000000000000000000000000280800000000000000000000000000\
                        00000000000000000000000000000000000000000000000000000000000000\
                        00000000000000000000000000000000000000100000000000000000000000\
                        000000000000000000000000000000000000000205c6ee304399dbdb9c8ef0\
                        30ab642b10820db8f560002000000000000000000140000000000000000000\
                        00000000000000000000000000000000000000000000000000000000000000\
                        00000000000000000000000000000000000000000000001000000000000000\
                        0000000000000000000000000000000000de0b6b3a76400000000000000000\
                        0000000000000000000000000000000000000000000000000a000000000000\
                        00000000000000000000000000000000000000000000000000000000000000\
                        00000000000000000000000000000000000000000000000000000020000000\
                        00000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000\
                        0000000000000000000ba100000625a3754423978a60c9317c58a424e3d000\
                        00000000000000000000000000000000000000000000000000000000000020\
                        000000000000000000000000000000000000000000000000de0b6b3a764000\
                        0fffffffffffffffffffffffffffffffffffffffffffffff3c9049e4e47ca5\
                        0ec",
                    "inputs": [
                        {
                            "amount": "1000000000000000000",
                            "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                        }
                    ],
                    "internalize": false,
                    "kind": "custom",
                    "outputs": [
                        {
                            "amount": "227598784442065388110",
                            "token": "0xba100000625a3754423978a60c9317c58a424e3d"
                        }
                    ],
                    "target": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                    "value": "0"
                }
            ],
            "prices": {
                "0xba100000625a3754423978a60c9317c58a424e3d": "1000000000000000000",
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "227598784442065388110"
            },
            "trades": [
                {
                    "executedAmount": "16000000000000000000",
                    "kind": "fulfillment",
                    "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                }
            ]
        })
    );
}
