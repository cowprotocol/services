//! This test ensures that the ParaSwap solver properly handles sell and buy
//! market orders, turning ParaSwap swap responses into CoW Protocol solutions.

use {
    crate::tests::{self, mock, paraswap},
    serde_json::json,
};

#[tokio::test]
async fn sell() {
    let api = mock::http::setup(vec![
        mock::http::Expectation::Get {
            path: mock::http::Path::exact(
                "prices?srcToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&destToken=0xe41d2489571d322189246dafa5ebde1f4699f498&srcDecimals=18&destDecimals=18&amount=1000000000000000000&side=SELL&excludeDEXS=UniswapV2&network=1&partner=cow",
            ),
            res: json!({
              "priceRoute": {
                "blockNumber": 17328561,
                "network": 1,
                "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "srcDecimals": 18,
                "srcAmount": "1000000000000000000",
                "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                "destDecimals": 18,
                "destAmount": "8116136957818361742974",
                "bestRoute": [
                  {
                    "percent": 100,
                    "swaps": [
                      {
                        "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "srcDecimals": 18,
                        "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                        "destDecimals": 18,
                        "swapExchanges": [
                          {
                            "exchange": "UniswapV3",
                            "srcAmount": "1000000000000000000",
                            "destAmount": "8116136957818361742974",
                            "percent": 100,
                            "poolAddresses": ["0x14424eeecbff345b38187d0b8b749e56faa68539"],
                            "data": {
                              "path": [
                                {
                                  "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                  "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                                  "fee": "3000"
                                }
                              ],
                              "gasUSD": "7.008815"
                            }
                          }
                        ]
                      }
                    ]
                  }
                ],
                "gasCostUSD": "12.768692",
                "gasCost": "242300",
                "side": "SELL",
                "tokenTransferProxy": "0x216b4b4ba9f3e719726886d34a177484278bfcae",
                "contractAddress": "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57",
                "contractMethod": "simpleSwap",
                "partnerFee": 0,
                "srcUSD": "1817.1676000000",
                "destUSD": "1824.5319365284",
                "partner": "anon",
                "maxImpactReached": false,
                "hmac": "c1d0a55d2d98fe3b366a6225055fb5ddf83b43da"
              }
            }),
        },
        mock::http::Expectation::Post {
            path: mock::http::Path::exact("transactions/1?ignoreChecks=true"),
            req: mock::http::RequestBody::Exact(json!({
              "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
              "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
              "srcAmount": "1000000000000000000",
              "destAmount": "8034975588240178125544",
              "srcDecimals": 18,
              "destDecimals": 18,
              "priceRoute": {
                "bestRoute": [
                  {
                    "percent": 100,
                    "swaps": [
                      {
                        "destDecimals": 18,
                        "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                        "srcDecimals": 18,
                        "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "swapExchanges": [
                          {
                            "data": {
                              "gasUSD": "7.008815",
                              "path": [
                                {
                                  "fee": "3000",
                                  "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                  "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                                }
                              ]
                            },
                            "destAmount": "8116136957818361742974",
                            "exchange": "UniswapV3",
                            "percent": 100,
                            "poolAddresses": ["0x14424eeecbff345b38187d0b8b749e56faa68539"],
                            "srcAmount": "1000000000000000000"
                          }
                        ]
                      }
                    ]
                  }
                ],
                "blockNumber": 17328561,
                "contractAddress": "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57",
                "contractMethod": "simpleSwap",
                "destAmount": "8116136957818361742974",
                "destDecimals": 18,
                "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                "destUSD": "1824.5319365284",
                "gasCost": "242300",
                "gasCostUSD": "12.768692",
                "hmac": "c1d0a55d2d98fe3b366a6225055fb5ddf83b43da",
                "maxImpactReached": false,
                "network": 1,
                "partner": "anon",
                "partnerFee": 0,
                "side": "SELL",
                "srcAmount": "1000000000000000000",
                "srcDecimals": 18,
                "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "srcUSD": "1817.1676000000",
                "tokenTransferProxy": "0x216b4b4ba9f3e719726886d34a177484278bfcae"
              },
              "userAddress": "0xe0b3700e0aadcb18ed8d4bff648bc99896a18ad1",
              "partner": "cow"
            })),
            res: json!({
              "from": "0xe0b3700e0aadcb18ed8d4bff648bc99896a18ad1",
              "to": "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57",
              "value": "0",
              "data": "0x54e3f31b0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000e41d2489571d322189246dafa5ebde1f4699f4980000000000000000000000000000000000000000000000000de0b6b3a76400000000000000000000000000000000000000000000000001b393afae6117259ae80000000000000000000000000000000000000000000001b7fa06c9ffcefa067e00000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000000220000000000000000000000000000000000000000000000000000000000000038000000000000000000000000000000000000000000000000000000000000003e000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000636f770100000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000042000000000000000000000000000000000000000000000000000000000646e405d64f39066556746efbd37c5513dae10dd000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000e592427a0aece92de3edee1f18e0157c058615640000000000000000000000000000000000000000000000000000000000000124c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000def171fe48cf0115b1d80b88dc8eab59176fee57000000000000000000000000000000000000000000000000000000006477267d0000000000000000000000000000000000000000000000000de0b6b3a76400000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002bc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189246dafa5ebde1f4699f49800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000124000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
              "gasPrice": "29000000000",
              "chainId": 1
            }),
        },
    ])
    .await;

    let engine = tests::SolverEngine::new("paraswap", paraswap::config(&api.address)).await;

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
                    "feeAmount": "1000000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await;

    assert_eq!(
        solution,
        json!({
          "solutions": [
            {
              "id": 0,
              "interactions": [
                {
                  "allowances": [
                    {
                      "amount": "1000000000000000000",
                      "spender": "0x216b4b4ba9f3e719726886d34a177484278bfcae",
                      "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    }
                  ],
                  "callData": "0x54e3f31b0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000e41d2489571d322189246dafa5ebde1f4699f4980000000000000000000000000000000000000000000000000de0b6b3a76400000000000000000000000000000000000000000000000001b393afae6117259ae80000000000000000000000000000000000000000000001b7fa06c9ffcefa067e00000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000000220000000000000000000000000000000000000000000000000000000000000038000000000000000000000000000000000000000000000000000000000000003e000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000636f770100000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000042000000000000000000000000000000000000000000000000000000000646e405d64f39066556746efbd37c5513dae10dd000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000e592427a0aece92de3edee1f18e0157c058615640000000000000000000000000000000000000000000000000000000000000124c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000def171fe48cf0115b1d80b88dc8eab59176fee57000000000000000000000000000000000000000000000000000000006477267d0000000000000000000000000000000000000000000000000de0b6b3a76400000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002bc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189246dafa5ebde1f4699f49800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000124000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
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
                      "amount": "8116136957818361742974",
                      "token": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                    }
                  ],
                  "target": "0xdef171fe48cf0115b1d80b88dc8eab59176fee57",
                  "value": "0"
                }
              ],
              "prices": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "8116136957818361742974",
                "0xe41d2489571d322189246dafa5ebde1f4699f498": "1000000000000000000"
              },
              "trades": [
                {
                  "executedAmount": "1000000000000000000",
                  "kind": "fulfillment",
                  "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                }
              ],
              "score": {
                "kind": "riskadjusted",
                    "successProbability": 0.5,
              }
            }
          ]
        }),
    );
}

#[tokio::test]
async fn buy() {
    let api = mock::http::setup(vec![
        mock::http::Expectation::Get {
            path: mock::http::Path::exact(
                "prices?srcToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&destToken=0xe41d2489571d322189246dafa5ebde1f4699f498&srcDecimals=18&destDecimals=18&amount=1000000000000000000000&side=BUY&excludeDEXS=UniswapV2&network=1&partner=cow",
            ),
            res: json!({
              "priceRoute": {
                "blockNumber": 17328689,
                "network": 1,
                "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "srcDecimals": 18,
                "srcAmount": "123703440917771661",
                "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                "destDecimals": 18,
                "destAmount": "1000000000000000000000",
                "bestRoute": [
                  {
                    "percent": 100,
                    "swaps": [
                      {
                        "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "srcDecimals": 18,
                        "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                        "destDecimals": 18,
                        "swapExchanges": [
                          {
                            "exchange": "SushiSwap",
                            "srcAmount": "123703440917771660",
                            "destAmount": "1000000000000000000000",
                            "percent": 100,
                            "poolAddresses": ["0x0BC5AE46c32D99C434b7383183ACa16DD6E9BdC8"],
                            "data": {
                              "router": "0xF9234CB08edb93c0d4a4d4c70cC3FfD070e78e07",
                              "path": [
                                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "0xe41d2489571d322189246dafa5ebde1f4699f498"
                              ],
                              "factory": "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac",
                              "initCode": "0xe18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303",
                              "feeFactor": 10000,
                              "pools": [
                                {
                                  "address": "0x0BC5AE46c32D99C434b7383183ACa16DD6E9BdC8",
                                  "fee": 30,
                                  "direction": true
                                }
                              ],
                              "gasUSD": "5.556256"
                            }
                          }
                        ]
                      }
                    ]
                  }
                ],
                "gasCostUSD": "6.601758",
                "gasCost": "106935",
                "side": "BUY",
                "tokenTransferProxy": "0x216b4b4ba9f3e719726886d34a177484278bfcae",
                "contractAddress": "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57",
                "contractMethod": "buyOnUniswapV2Fork",
                "partnerFee": 0,
                "srcUSD": "224.6837967734",
                "destUSD": "224.7140000000",
                "partner": "anon",
                "maxImpactReached": false,
                "hmac": "6bb84509b20ea5ec6dac9f7758f72cc68045a3cb"
              }
            }),
        },
        mock::http::Expectation::Post {
            path: mock::http::Path::exact("transactions/1?ignoreChecks=true"),
            req: mock::http::RequestBody::Exact(json!({
              "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
              "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
              "srcAmount": "124940475326949378",
              "destAmount": "1000000000000000000000",
              "srcDecimals": 18,
              "destDecimals": 18,
              "priceRoute": {
                "bestRoute": [
                  {
                    "percent": 100,
                    "swaps": [
                      {
                        "destDecimals": 18,
                        "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                        "srcDecimals": 18,
                        "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "swapExchanges": [
                          {
                            "data": {
                              "factory": "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac",
                              "feeFactor": 10000,
                              "gasUSD": "5.556256",
                              "initCode": "0xe18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303",
                              "path": [
                                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "0xe41d2489571d322189246dafa5ebde1f4699f498"
                              ],
                              "pools": [
                                {
                                  "address": "0x0BC5AE46c32D99C434b7383183ACa16DD6E9BdC8",
                                  "direction": true,
                                  "fee": 30
                                }
                              ],
                              "router": "0xF9234CB08edb93c0d4a4d4c70cC3FfD070e78e07"
                            },
                            "destAmount": "1000000000000000000000",
                            "exchange": "SushiSwap",
                            "percent": 100,
                            "poolAddresses": ["0x0BC5AE46c32D99C434b7383183ACa16DD6E9BdC8"],
                            "srcAmount": "123703440917771660"
                          }
                        ]
                      }
                    ]
                  }
                ],
                "blockNumber": 17328689,
                "contractAddress": "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57",
                "contractMethod": "buyOnUniswapV2Fork",
                "destAmount": "1000000000000000000000",
                "destDecimals": 18,
                "destToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                "destUSD": "224.7140000000",
                "gasCost": "106935",
                "gasCostUSD": "6.601758",
                "hmac": "6bb84509b20ea5ec6dac9f7758f72cc68045a3cb",
                "maxImpactReached": false,
                "network": 1,
                "partner": "anon",
                "partnerFee": 0,
                "side": "BUY",
                "srcAmount": "123703440917771661",
                "srcDecimals": 18,
                "srcToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "srcUSD": "224.6837967734",
                "tokenTransferProxy": "0x216b4b4ba9f3e719726886d34a177484278bfcae"
              },
              "userAddress": "0xe0b3700e0aadcb18ed8d4bff648bc99896a18ad1",
              "partner": "cow"
            })),
            res: json!({
              "from": "0xe0b3700e0aadcb18ed8d4bff648bc99896a18ad1",
              "to": "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57",
              "value": "0",
              "data": "0xb2f1e6db000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000001bbe0b349ee680200000000000000000000000000000000000000000000003635c9adc5dea00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000004de40bc5ae46c32d99c434b7383183aca16dd6e9bdc8",
              "gasPrice": "34000000000",
              "chainId": 1
            }),
        },
    ])
    .await;

    let engine = tests::SolverEngine::new("paraswap", paraswap::config(&api.address)).await;

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
                    "sellAmount": "200000000000000000",
                    "buyAmount": "1000000000000000000000",
                    "feeAmount": "1000000000000000",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await;

    assert_eq!(
        solution,
        json!({
          "solutions": [
            {
              "id": 0,
              "interactions": [
                {
                  "allowances": [
                    {
                      "amount": "123703440917771661",
                      "spender": "0x216b4b4ba9f3e719726886d34a177484278bfcae",
                      "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    }
                  ],
                  "callData": "0xb2f1e6db000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000001bbe0b349ee680200000000000000000000000000000000000000000000003635c9adc5dea00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000004de40bc5ae46c32d99c434b7383183aca16dd6e9bdc8",
                  "inputs": [
                    {
                      "amount": "123703440917771661",
                      "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    }
                  ],
                  "internalize": true,
                  "kind": "custom",
                  "outputs": [
                    {
                      "amount": "1000000000000000000000",
                      "token": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                    }
                  ],
                  "target": "0xdef171fe48cf0115b1d80b88dc8eab59176fee57",
                  "value": "0"
                }
              ],
              "prices": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1000000000000000000000",
                "0xe41d2489571d322189246dafa5ebde1f4699f498": "123703440917771661"
              },
              "trades": [
                {
                  "executedAmount": "1000000000000000000000",
                  "kind": "fulfillment",
                  "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                }
              ],
              "score": {
                "kind": "riskadjusted",
                    "successProbability": 0.5,
              }
            }
          ]
        }),
    );
}
