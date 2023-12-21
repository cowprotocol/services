//! This test ensures that the 0x solver properly handles sell and buy market
//! orders, turning 0x swap responses into CoW Protocol solutions.

use {
    crate::tests::{self, mock, zeroex},
    serde_json::json,
};

#[tokio::test]
async fn sell() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "swap/v1/quote\
             ?sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
             &buyToken=0xe41d2489571d322189246dafa5ebde1f4699f498\
             &sellAmount=1000000000000000000\
             &slippagePercentage=0.01\
             &gasPrice=15000000000\
             &takerAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41\
             &skipValidation=true\
             &intentOnFilling=false\
             &affiliateAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41\
             &enableSlippageProtection=false",
        ),
        res: json!({
            "chainId": 1,
            "price": "5876.422636675954",
            "guaranteedPrice": "5817.65841030919446",
            "estimatedPriceImpact": "0.3623",
            "to": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            "data": "0x6af479b2\
                       0000000000000000000000000000000000000000000000000000000000000080\
                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                       00000000000000000000000000000000000000000000013b603a9ce6a341ab60\
                       0000000000000000000000000000000000000000000000000000000000000000\
                       000000000000000000000000000000000000000000000000000000000000002b\
                       c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189\
                       246dafa5ebde1f4699f498000000000000000000000000000000000000000000\
                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                       0560ab4100000000000000000000000000000000000000000000009c6fd65477\
                       63f8730a",
            "value": "0",
            "gas": "127886",
            "estimatedGas": "127886",
            "gasPrice": "15000000000",
            "protocolFee": "0",
            "minimumProtocolFee": "0",
            "buyTokenAddress": "0xe41d2489571d322189246dafa5ebde1f4699f498",
            "sellTokenAddress": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "buyAmount": "5876422636675954000000",
            "sellAmount": "1000000000000000000",
            "sources": [
                {
                    "name": "Uniswap_V3",
                    "proportion": "1",
                },
            ],
            "orders": [
                {
                    "type": 0,
                    "source": "Uniswap_V3",
                    "makerToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "takerToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "makerAmount": "5876422636675954000000",
                    "takerAmount": "1000000000000000000",
                    "fillData": {
                        "router": "0xe592427a0aece92de3edee1f18e0157c05861564",
                        "tokenAddressPath": [
                            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                            "0xe41d2489571d322189246dafa5ebde1f4699f498",
                        ],
                        "path": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189246dafa5ebde1f4699f498",
                        "gasUsed": 67886,
                    },
                    "fill": {
                        "input": "1000000000000000000",
                        "output": "5876422636675954000000",
                        "adjustedOutput": "5867409814019629688976",
                        "gas": 101878,
                    },
                },
            ],
            "allowanceTarget": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            "decodedUniqueId": "9c6fd65477-1677226762",
            "sellTokenToEthRate": "1",
            "buyTokenToEthRate": "5897.78797929831826547",
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("zeroex", zeroex::config(&api.address)).await;

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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "5876422636675954000000",
                    "0xe41d2489571d322189246dafa5ebde1f4699f498": "1000000000000000000",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000",
                    }
                ],
                "interactions": [
                    {
                        "kind": "custom",
                        "internalize": false,
                        "target": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                        "value": "0",
                        "callData": "0x6af479b2\
                                       0000000000000000000000000000000000000000000000000000000000000080\
                                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                                       00000000000000000000000000000000000000000000013b603a9ce6a341ab60\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       000000000000000000000000000000000000000000000000000000000000002b\
                                       c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189\
                                       246dafa5ebde1f4699f498000000000000000000000000000000000000000000\
                                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                                       0560ab4100000000000000000000000000000000000000000000009c6fd65477\
                                       63f8730a",
                        "allowances": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "spender": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                                "amount": "1000000000000000000",
                            },
                        ],
                        "inputs": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "amount": "1000000000000000000",
                            },
                        ],
                        "outputs": [
                            {
                                "token": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                                "amount": "5876422636675954000000",
                            },
                        ],
                    },
                ],
                "score": {
                    "kind": "riskadjusted",
                    "successProbability": 0.5,
                }
            }]
        }),
    );
}

#[tokio::test]
async fn buy() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "swap/v1/quote?sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
             buyToken=0xe41d2489571d322189246dafa5ebde1f4699f498&buyAmount=1000000000000000000000&\
             slippagePercentage=0.01&gasPrice=15000000000&\
             takerAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41&skipValidation=true&\
             intentOnFilling=false&affiliateAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41&\
             enableSlippageProtection=false",
        ),
        res: json!({
            "chainId": 1,
            "price": "0.000169864053551884",
            "guaranteedPrice": "0.000171562694087402",
            "estimatedPriceImpact": "0.1819",
            "to": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            "data": "0xd9627aa4\
                       0000000000000000000000000000000000000000000000000000000000000080\
                       0000000000000000000000000000000000000000000000000261835c7dc9e181\
                       00000000000000000000000000000000000000000000003635c9adc5dea00000\
                       0000000000000000000000000000000000000000000000000000000000000000\
                       0000000000000000000000000000000000000000000000000000000000000002\
                       000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                       000000000000000000000000e41d2489571d322189246dafa5ebde1f4699f498\
                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                       0560ab4100000000000000000000000000000000000000000000003111411442\
                       63f8bfab",
            "value": "0",
            "gas": "111000",
            "estimatedGas": "111000",
            "gasPrice": "15000000000",
            "protocolFee": "0",
            "minimumProtocolFee": "0",
            "buyTokenAddress": "0xe41d2489571d322189246dafa5ebde1f4699f498",
            "sellTokenAddress": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "buyAmount": "1000000000000000000000",
            "sellAmount": "169864053551883026",
            "sources": [
                {
                    "name": "Uniswap_V2",
                    "proportion": "1",
                },
            ],
            "orders": [
                {
                    "type": 0,
                    "source": "Uniswap_V2",
                    "makerToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "takerToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "makerAmount": "1000000000000000000000",
                    "takerAmount": "169864053551883026",
                    "fillData": {
                        "tokenAddressPath": [
                            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                            "0xe41d2489571d322189246dafa5ebde1f4699f498",
                        ],
                        "router": "0xf164fc0ec4e93095b804a4795bbe1e041497b92a",
                    },
                    "fill": {
                        "input": "1000000000000000000000",
                        "output": "169864053551883026",
                        "adjustedOutput": "169864053551883026",
                        "gas": 90000,
                    },
                },
            ],
            "allowanceTarget": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
            "decodedUniqueId": "3111411442-1677246379",
            "sellTokenToEthRate": "1",
            "buyTokenToEthRate": "5897.78797929831826547",
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("zeroex", zeroex::config(&api.address)).await;

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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1000000000000000000000",
                    "0xe41d2489571d322189246dafa5ebde1f4699f498": "169864053551883026",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000000",
                    }
                ],
                "interactions": [
                    {
                        "kind": "custom",
                        "internalize": true,
                        "target": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                        "value": "0",
                        "callData": "0xd9627aa4\
                                       0000000000000000000000000000000000000000000000000000000000000080\
                                       0000000000000000000000000000000000000000000000000261835c7dc9e181\
                                       00000000000000000000000000000000000000000000003635c9adc5dea00000\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000002\
                                       000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                                       000000000000000000000000e41d2489571d322189246dafa5ebde1f4699f498\
                                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                                       0560ab4100000000000000000000000000000000000000000000003111411442\
                                       63f8bfab",
                        "allowances": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "spender": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                                "amount": "171562694087401857",
                            },
                        ],
                        "inputs": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "amount": "169864053551883026",
                            },
                        ],
                        "outputs": [
                            {
                                "token": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                                "amount": "1000000000000000000000",
                            },
                        ],
                    },
                ],
                "score": {
                    "kind": "riskadjusted",
                    "successProbability": 0.5,
                }
            }]
        }),
    );
}
