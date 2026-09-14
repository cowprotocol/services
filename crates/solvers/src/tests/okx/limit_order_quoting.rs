//! This test ensures that dex solvers can quote limit-class orders even when
//! the auction carries no token reference prices, which is the shape of quote
//! auctions. Without a reference price the solver cannot convert its gas cost
//! into a sell-token fee, so it falls back to a zero fee (which is never
//! charged on the quote path) and skips the gas simulation (whose result the
//! orderbook replaces with its own measurement).

use {
    crate::tests::{self, mock},
    serde_json::json,
};

#[tokio::test]
async fn sell() {
    let api = mock::http::setup(vec![
        mock::http::Expectation::Get {
            path: mock::http::Path::exact(
                "swap?chainIndex=1\
                &amount=1000000000000000000\
                &fromTokenAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                &toTokenAddress=0xe41d2489571d322189246dafa5ebde1f4699f498\
                &slippagePercent=0.01\
                &userWalletAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41\
                &swapReceiverAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41\
                &swapMode=exactIn\
                &priceImpactProtectionPercent=1"
            ),
            res: json!(
              {
                "code":"0",
                "data":[
                   {
                      "routerResult":{
                         "chainId":"1",
                         "dexRouterList":[
                            {
                               "router":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2--0xe41d2489571d322189246dafa5ebde1f4699f498",
                               "routerPercent":"100",
                               "subRouterList":[
                                  {
                                     "dexProtocol":[
                                        {
                                           "dexName":"Uniswap V3",
                                           "percent":"100"
                                        }
                                     ],
                                     "fromToken":{
                                        "decimal":"18",
                                        "isHoneyPot":false,
                                        "taxRate":"0",
                                        "tokenContractAddress":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                        "tokenSymbol":"WETH",
                                        "tokenUnitPrice":"3315.553196726842565048"
                                     },
                                     "toToken":{
                                        "decimal":"18",
                                        "isHoneyPot":false,
                                        "taxRate":"0",
                                        "tokenContractAddress":"0xe41d2489571d322189246dafa5ebde1f4699f498",
                                        "tokenSymbol":"ZRX",
                                        "tokenUnitPrice":"0.504455838152300152"
                                     }
                                  }
                               ]
                            }
                         ],
                         "estimateGasFee":"135000",
                         "fromToken":{
                            "decimal":"18",
                            "isHoneyPot":false,
                            "taxRate":"0",
                            "tokenContractAddress":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                            "tokenSymbol":"WETH",
                            "tokenUnitPrice":"3315.553196726842565048"
                         },
                         "fromTokenAmount":"1000000000000000000",
                         "priceImpactPercentage":"-0.25",
                         "quoteCompareList":[
                            {
                               "amountOut":"6556.259156432631386442",
                               "dexLogo":"https://static.okx.com/cdn/wallet/logo/UNI.png",
                               "dexName":"Uniswap V3",
                               "tradeFee":"2.3554356342513966"
                            },
                            {
                               "amountOut":"6375.198002761542738881",
                               "dexLogo":"https://static.okx.com/cdn/wallet/logo/UNI.png",
                               "dexName":"Uniswap V2",
                               "tradeFee":"3.34995290204643072"
                            },
                            {
                               "amountOut":"4456.799978982369793812",
                               "dexLogo":"https://static.okx.com/cdn/wallet/logo/UNI.png",
                               "dexName":"Uniswap V1",
                               "tradeFee":"4.64638467513839940864"
                            },
                            {
                               "amountOut":"2771.072269036022134969",
                               "dexLogo":"https://static.okx.com/cdn/wallet/logo/SUSHI.png",
                               "dexName":"SushiSwap",
                               "tradeFee":"3.34995290204643072"
                            }
                         ],
                         "toToken":{
                            "decimal":"18",
                            "isHoneyPot":false,
                            "taxRate":"0",
                            "tokenContractAddress":"0xe41d2489571d322189246dafa5ebde1f4699f498",
                            "tokenSymbol":"ZRX",
                            "tokenUnitPrice":"0.504455838152300152"
                         },
                         "toTokenAmount":"6556259156432631386442",
                         "tradeFee":"2.3554356342513966"
                      },
                      "tx":{
                         "data":"0x0d5f0e3b00000000000000000001a0cf2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a0000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000000000000000000000000015fdc8278903f7f31c10000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000100000000000000000000000014424eeecbff345b38187d0b8b749e56faa68539",
                         "from":"0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                         "gas":"202500",
                         "gasPrice":"6756286873",
                         "maxPriorityFeePerGas":"1000000000",
                         "minReceiveAmount":"6490696564868305072578",
                         "signatureData":[
                            ""
                         ],
                         "slippage":"0.01",
                         "to":"0x7D0CcAa3Fac1e5A943c5168b6CEd828691b46B36",
                         "value":"0"
                      }
                   }
                ],
                "msg":""
             }),
        },
        mock::http::Expectation::Get {
         path: mock::http::Path::exact(
             "approve-transaction?chainIndex=1\
             &tokenContractAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
             &approveAmount=1000000000000000000"
         ),
         res: json!(
           {
             "code":"0",
             "data":[{"data":"0x095ea7b300000000000000000000000040aa958dd87fc8305b97f2ba922cddca374bcd7f000000000000000000000000000000000000000000000000000009184e72a000","dexContractAddress":"0x40aA958dd87FC8305b97f2BA922CDdCa374bcD7f","gasLimit":"70000","gasPrice":"7424402761"}],
             "msg":""
           }
         )
      },
    ])
    .await;

    let engine = tests::SolverEngine::new("okx", super::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xe41d2489571d322189246dafa5ebde1f4699f498": {
                    "decimals": 18,
                    "symbol": "ZRX",
                    "referencePrice": null,
                    "availableBalance": "1583034704488033979459",
                    "trusted": true,
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": null,
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
                    "buyAmount": "1",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "1",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "limit",
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
                 "gas":410141,
                 "id":0,
                 "interactions":[
                    {
                       "allowances":[
                          {
                             "amount":"1000000000000000000",
                             "spender":"0x40aa958dd87fc8305b97f2ba922cddca374bcd7f",
                             "token":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                          }
                       ],
                       "callData":"0x0d5f0e3b00000000000000000001a0cf2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a0000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000000000000000000000000015fdc8278903f7f31c10000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000100000000000000000000000014424eeecbff345b38187d0b8b749e56faa68539",
                       "inputs":[
                          {
                             "amount":"1000000000000000000",
                             "token":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                          }
                       ],
                       "internalize":false,
                       "kind":"custom",
                       "outputs":[
                          {
                             "amount":"6556259156432631386442",
                             "token":"0xe41d2489571d322189246dafa5ebde1f4699f498"
                          }
                       ],
                       "target":"0x7d0ccaa3fac1e5a943c5168b6ced828691b46b36",
                       "value":"0"
                    }
                 ],
                 "postInteractions":[],
                 "preInteractions":[],
                 "prices":{
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2":"6556259156432631386442",
                    "0xe41d2489571d322189246dafa5ebde1f4699f498":"1000000000000000000"
                 },
                 "trades":[
                    {
                       "executedAmount":"1000000000000000000",
                       "fee":"0",
                       "kind":"fulfillment",
                       "order":"0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                    }
                 ]
              }
           ]
        }),
    );
}
