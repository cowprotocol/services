//! This test verifies that the Bitget solver does not generate solutions when
//! the swap returned from the API does not satisfy an order's limit price.
//!
//! The actual test case is a modified version of the [`super::market_order`]
//! test with an exuberant buy amount.

use {
    crate::tests::{self, mock},
    serde_json::json,
};

#[tokio::test]
async fn sell() {
    let api = mock::http::setup(vec![
        mock::http::Expectation::Post {
            path: mock::http::Path::exact("bgw-pro/swapx/pro/swap"),
            req: mock::http::RequestBody::Any,
            res: json!({
                "status": 0,
                "data": {
                    "outAmount": "6556.259156432631386442",
                    "minAmount": "6490.696564868305072577",
                    "gasFee": {
                        "gasLimit": "202500"
                    },
                    "swapTransaction": {
                        "to": "0x7D0CcAa3Fac1e5A943c5168b6CEd828691b46B36",
                        "data": "0x0d5f0e3b00000000000000000001a0cf2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                             0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                             00000000000000000000000000000000000000000000015fdc8278903f7f31c1\
                             0000000000000000000000000000000000000000000000000000000000000080\
                             0000000000000000000000000000000000000000000000000000000000000001\
                             00000000000000000000000014424eeecbff345b38187d0b8b749e56faa68539"
                    }
                }
            }),
        },
    ])
    .await;

    let engine = tests::SolverEngine::new("bitget", super::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xe41d2489571d322189246dafa5ebde1f4699f498": {
                    "decimals": 18,
                    "symbol": "ZRX",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true,
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "482725140468789680",
                    "trusted": true,
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "sellAmount": "1000000000000000000",
                    // Way too much...
                    "buyAmount": "1000000000000000000000000000000000000",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "1000000000000000000000000000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "preInteractions": [],
                    "postInteractions": [],
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "validTo": 0,
                    "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "signingScheme": "presign",
                    "signature": "0x",
                }
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
            "surplusCapturingJitOrderOwners": []
        }))
        .await;

    assert_eq!(solution, json!({ "type": "solutions", "solutions": [] }),);
}
