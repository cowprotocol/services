//! This test verifies that the Balancer SOR solver does not generate solutions
//! when the swap returned from the API does not satisfy an orders limit price.
//!
//! The actual test case is a modified version of the [`super::market_order`]
//! test cases with exuberant limit prices.

use {
    crate::tests::{self, balancer, mock},
    serde_json::json,
};

#[tokio::test]
async fn sell() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: json!({
            "sellToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "buyToken": "0xba100000625a3754423978a60c9317c58a424e3d",
            "orderKind": "sell",
            "amount": "1000000000000000000",
            "gasPrice": "15000000000",
        }),
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

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api.address)).await;

    let solution = engine
        .solve(json!({
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
                    "sellAmount": "1000000000000000000",
                    // Way too much...
                    "buyAmount": "1000000000000000000000000000000000000",
                    "feeAmount": "1000000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "prices": {},
            "trades": [],
            "interactions": [],
        }),
    );
}

#[tokio::test]
async fn buy() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: json!({
            "sellToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "buyToken": "0xba100000625a3754423978a60c9317c58a424e3d",
            "orderKind": "buy",
            "amount": "100000000000000000000",
            "gasPrice": "15000000000",
        }),
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
                    "amount": "100000000000000000000",
                    "userData": "0x",
                    "returnAmount": "439470293178110675"
                }
            ],
            "swapAmount": "100000000000000000000",
            "swapAmountForSwaps": "100000000000000000000",
            "returnAmount": "439470293178110675",
            "returnAmountFromSwaps": "439470293178110675",
            "returnAmountConsideringFees": "440745919677086983",
            "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
            "marketSp": "0.004394663712203829"
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api.address)).await;

    let solution = engine
        .solve(json!({
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
                    // Way too little...
                    "sellAmount": "1",
                    "buyAmount": "100000000000000000000",
                    "feeAmount": "1000000000000000",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "prices": {},
            "trades": [],
            "interactions": [],
        }),
    );
}
