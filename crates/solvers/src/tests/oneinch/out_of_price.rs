//! This test verifies that the 1inch solver does not generate solutions when
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
        mock::http::Expectation::Get {
            path: mock::http::Path::exact("liquidity-sources"),
            res: json!(
                {
                  "protocols": [
                    {
                      "id": "UNISWAP_V1",
                      "title": "Uniswap V1",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/uniswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/uniswap_color.png"
                    },
                    {
                      "id": "UNISWAP_V2",
                      "title": "Uniswap V2",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/uniswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/uniswap_color.png"
                    },
                    {
                      "id": "SUSHI",
                      "title": "SushiSwap",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/sushiswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/sushiswap_color.png"
                    },
                    {
                      "id": "UNISWAP_V3",
                      "title": "Uniswap V3",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/uniswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/uniswap_color.png"
                    },
                  ]
                }
            ),
        },
        mock::http::Expectation::Get {
            path: mock::http::Path::exact("approve/spender"),
            res: json!({ "address": "0x1111111254eeb25477b68fb85ed929f73a960582" }),
        },
        mock::http::Expectation::Get {
            path: mock::http::Path::exact(
                "swap\
                ?fromTokenAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                &toTokenAddress=0xe41d2489571d322189246dafa5ebde1f4699f498\
                &amount=1000000000000000000\
                &fromAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41\
                &slippage=1\
                &protocols=UNISWAP_V1%2CUNISWAP_V2%2CSUSHI\
                &referrerAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41\
                &disableEstimate=true\
                &gasPrice=15000000000"
            ),
            res: json!(
              {
                "fromToken": {
                  "symbol": "WETH",
                  "name": "Wrapped Ether",
                  "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                  "decimals": 18,
                  "logoURI": "https://tokens.1inch.io/0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2.png",
                  "wrappedNative": true,
                  "tags": ["tokens", "PEG:ETH"]
                },
                "toToken": {
                  "symbol": "ZRX",
                  "name": "0x Protocol",
                  "address": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                  "decimals": 18,
                  "logoURI": "https://tokens.1inch.io/0xe41d2489571d322189246dafa5ebde1f4699f498.png",
                  "tags": ["tokens"]
                },
                "toTokenAmount": "7849120067437052861364",
                "fromTokenAmount": "1000000000000000000",
                "protocols": [
                  [
                    [
                      {
                        "name": "SUSHI",
                        "part": 10,
                        "fromTokenAddress": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "toTokenAddress": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                      },
                      {
                        "name": "UNISWAP_V1",
                        "part": 10,
                        "fromTokenAddress": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "toTokenAddress": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                      },
                      {
                        "name": "UNISWAP_V2",
                        "part": 80,
                        "fromTokenAddress": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "toTokenAddress": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                      }
                    ]
                  ]
                ],
                "tx": {
                  "from": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                  "to": "0x1111111254eeb25477b68fb85ed929f73a960582",
                  "data": "0x12aa3caf0000000000000000000000001136b25047e142fa3018184793aec68fbb173ce4000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000e41d2489571d322189246dafa5ebde1f4699f4980000000000000000000000001136b25047e142fa3018184793aec68fbb173ce40000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab410000000000000000000000000000000000000000000000000de0b6b3a76400000000000000000000000000000000000000000000000001a53f2377c3b1e2f64e0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000001600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000022000000000000000000000000000000000000000000000000000000000020200a0c9e75c48000000000000050028050000000000000000000000000000000000000001d400015a00011e00008f0c20c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20bc5ae46c32d99c434b7383183aca16dd6e9bdc86ae40711b8002dc6c00bc5ae46c32d99c434b7383183aca16dd6e9bdc81111111254eeb25477b68fb85ed929f73a96058200000000000000000000000000000000000000000000002a1e26c62c19f03aabc02aaa39b223fe8d0a0e5c4f27ead9083c756cc20c20c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2c6f348dd3b91a56d117ec0071c1e9b83c0996de46ae40711b8002dc6c0c6f348dd3b91a56d117ec0071c1e9b83c0996de41111111254eeb25477b68fb85ed929f73a9605820000000000000000000000000000000000000000000001508d106e5f82eea57ec02aaa39b223fe8d0a0e5c4f27ead9083c756cc24101c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200042e1a7d4d00000000000000000000000000000000000000000000000000000000000000004040ae76c84c9262cdb9abc0c2c8888e62db8e22a0bfad65d76d00000000000000000000000000000000000000000000002a93ec43381504162400000000000000000000000000000000000000000000000000000000646f5dde0000000000000000000000001111111254eeb25477b68fb85ed929f73a960582cfee7c08",
                  "value": "0",
                  "gas": 0,
                  "gasPrice": "15000000000"
                }
              }

            ),
        }
    ])
    .await;

    let engine = tests::SolverEngine::new("oneinch", super::config(&api.address)).await;

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

    assert_eq!(solution, json!({ "solutions": [] }),);
}
