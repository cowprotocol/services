//! This test ensures that the Bitget solver properly handles market sell
//! orders, turning Bitget swap responses into CoW Protocol solutions.

use {
    crate::tests::{self, mock},
    serde_json::json,
};

#[tokio::test]
async fn sell() {
    let api = mock::http::setup(vec![
        mock::http::Expectation::Post {
            path: mock::http::Path::exact("bgw-pro/swapx/pro/swap"),
            req: mock::http::RequestBody::Partial(
                json!({
                    "fromContract": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "fromAmount": "1",
                    "fromChain": "eth",
                    "toContract": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "toChain": "eth",
                    "market": "bgwevmaggregator",
                    "slippage": 1.0,
                    "requestMod": "rich",
                    "feeRate": 0.0
                }),
                vec!["fromAddress", "toAddress"],
            ),
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
                    "buyAmount": "200000000000000000000",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "200000000000000000000",
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

    assert_eq!(
        solution,
        json!({
           "solutions":[
              {
                 "gas": 410141,
                 "id": 0,
                 "interactions":[
                    {
                       "allowances":[
                          {
                             "amount": "1000000000000000000",
                             "spender": "0x7d0ccaa3fac1e5a943c5168b6ced828691b46b36",
                             "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                          }
                       ],
                       "callData": "0x0d5f0e3b00000000000000000001a0cf2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a0000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000000000000000000000000015fdc8278903f7f31c10000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000100000000000000000000000014424eeecbff345b38187d0b8b749e56faa68539",
                       "inputs":[
                          {
                             "amount": "1000000000000000000",
                             "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                          }
                       ],
                       "internalize": false,
                       "kind": "custom",
                       "outputs":[
                          {
                             "amount": "6556259156432631386442",
                             "token": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                          }
                       ],
                       "target": "0x7d0ccaa3fac1e5a943c5168b6ced828691b46b36",
                       "value": "0"
                    }
                 ],
                 "postInteractions": [],
                 "preInteractions": [],
                 "prices":{
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "6556259156432631386442",
                    "0xe41d2489571d322189246dafa5ebde1f4699f498": "1000000000000000000"
                 },
                 "trades":[
                    {
                       "executedAmount": "1000000000000000000",
                       "kind": "fulfillment",
                       "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                    }
                 ]
              }
           ]
        }),
    );
}

#[tokio::test]
async fn buy() {
    let api = mock::http::setup(vec![
        mock::http::Expectation::Post {
            path: mock::http::Path::exact("bgw-pro/swapx/pro/swapr"),
            req: mock::http::RequestBody::Partial(
                json!({
                    "fromContract": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "toContract": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "fromChain": "eth",
                    "amount": "200",
                    "requestMode": "minAmountOut",
                    "slippage": "1",
                    "feeRate": 0.0
                }),
                vec!["fromAddress", "toAddress"],
            ),
            // Mock the reverse-quote response. `expectedAmountOut` is a small
            // overshoot above the requested 200, simulating BitGet's recursion
            // landing slightly above the target. We report the exact buy
            // amount in the solution (CoW exact-out semantics) and the
            // overshoot becomes positive surplus on chain.
            res: json!({
                "status": 0,
                "data": {
                    "amountIn": "0.99",
                    "expectedAmountOut": "200.5",
                    "minAmountOut": "200",
                    "priceImpact": "0",
                    "recommendSlippage": 0.5,
                    "expiresAt": 0,
                    "market": "bgwevmaggregator",
                    "txs": [
                        {
                            "chainId": 1,
                            "to": "0x7D0CcAa3Fac1e5A943c5168b6CEd828691b46B36",
                            "calldata": "0x0d5f0e3b00000000000000000001a0cf2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                 0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                                 00000000000000000000000000000000000000000000015fdc8278903f7f31c1\
                                 0000000000000000000000000000000000000000000000000000000000000080\
                                 0000000000000000000000000000000000000000000000000000000000000001\
                                 00000000000000000000000014424eeecbff345b38187d0b8b749e56faa68539",
                            "function": "swap",
                            "gasLimit": "202500",
                            "gasPrice": "0",
                            "nonce": 0,
                            "value": "0"
                        }
                    ],
                    "fee": {}
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
                    "buyAmount": "200000000000000000000",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "200000000000000000000",
                    "kind": "buy",
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

    assert_eq!(
        solution,
        json!({
           "solutions":[
              {
                 "gas": 410141,
                 "id": 0,
                 "interactions":[
                    {
                       "allowances":[
                          {
                             "amount": "990000000000000000",
                             "spender": "0x7d0ccaa3fac1e5a943c5168b6ced828691b46b36",
                             "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                          }
                       ],
                       "callData": "0x0d5f0e3b00000000000000000001a0cf2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a0000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000000000000000000000000015fdc8278903f7f31c10000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000100000000000000000000000014424eeecbff345b38187d0b8b749e56faa68539",
                       "inputs":[
                          {
                             "amount": "990000000000000000",
                             "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                          }
                       ],
                       "internalize": true,
                       "kind": "custom",
                       "outputs":[
                          {
                             "amount": "200000000000000000000",
                             "token": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                          }
                       ],
                       "target": "0x7d0ccaa3fac1e5a943c5168b6ced828691b46b36",
                       "value": "0"
                    }
                 ],
                 "postInteractions": [],
                 "preInteractions": [],
                 "prices":{
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "200000000000000000000",
                    "0xe41d2489571d322189246dafa5ebde1f4699f498": "990000000000000000"
                 },
                 "trades":[
                    {
                       "executedAmount": "200000000000000000000",
                       "kind": "fulfillment",
                       "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                    }
                 ]
              }
           ]
        }),
    );
}
