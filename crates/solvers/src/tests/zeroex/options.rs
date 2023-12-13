//! This tests the 0x solver's handling of optional configuration fields.

use {
    crate::tests::{self, mock},
    serde_json::json,
};

#[tokio::test]
async fn test() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "swap/v1/quote\
             ?sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
             &buyToken=0xe41d2489571d322189246dafa5ebde1f4699f498\
             &sellAmount=1000000000000000000\
             &slippagePercentage=0.1\
             &gasPrice=15000000000\
             &takerAddress=0x9008d19f58aabd9ed0d60971565aa8510560ab41\
             &excludedSources=Uniswap_V2%2CBalancer_V2\
             &skipValidation=true\
             &intentOnFilling=true\
             &affiliateAddress=0x0123456789012345678901234567890123456789\
             &enableSlippageProtection=true",
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
                    "name": "0x",
                    "proportion": "0",
                },
                {
                    "name": "Uniswap",
                    "proportion": "0",
                },
                {
                    "name": "Curve",
                    "proportion": "0",
                },
                {
                    "name": "Balancer",
                    "proportion": "0",
                },
                {
                    "name": "Bancor",
                    "proportion": "0",
                },
                {
                    "name": "BancorV3",
                    "proportion": "0",
                },
                {
                    "name": "mStable",
                    "proportion": "0",
                },
                {
                    "name": "SushiSwap",
                    "proportion": "0",
                },
                {
                    "name": "Shell",
                    "proportion": "0",
                },
                {
                    "name": "DODO",
                    "proportion": "0",
                },
                {
                    "name": "DODO_V2",
                    "proportion": "0",
                },
                {
                    "name": "CryptoCom",
                    "proportion": "0",
                },
                {
                    "name": "Lido",
                    "proportion": "0",
                },
                {
                    "name": "MakerPsm",
                    "proportion": "0",
                },
                {
                    "name": "KyberDMM",
                    "proportion": "0",
                },
                {
                    "name": "Component",
                    "proportion": "0",
                },
                {
                    "name": "Saddle",
                    "proportion": "0",
                },
                {
                    "name": "Uniswap_V3",
                    "proportion": "1",
                },
                {
                    "name": "Curve_V2",
                    "proportion": "0",
                },
                {
                    "name": "ShibaSwap",
                    "proportion": "0",
                },
                {
                    "name": "Synapse",
                    "proportion": "0",
                },
                {
                    "name": "Synthetix",
                    "proportion": "0",
                },
                {
                    "name": "Aave_V2",
                    "proportion": "0",
                },
                {
                    "name": "Compound",
                    "proportion": "0",
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

    let config = tests::Config::String(format!(
        r"
node-url = 'http://localhost:8545'
relative-slippage = '0.1'
risk-parameters = [0,0,0,0]
[dex]
endpoint = 'http://{}/swap/v1/'
api-key = 'abc123'
excluded-sources = ['Uniswap_V2', 'Balancer_V2']
affiliate = '0x0123456789012345678901234567890123456789'
enable-rfqt = true
enable-slippage-protection = true
        ",
        api.address
    ));
    let engine = tests::SolverEngine::new("zeroex", config).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xe41d2489571d322189246dafa5ebde1f4699f498": {
                    "decimals": 18,
                    "symbol": "ZRX",
                    "referencePrice": "168664736580767",
                    "availableBalance": "297403065984541243067",
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
                    "buyAmount": "5000000000000000000000",
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
                    "riskadjusted": 0.5
                }
            }]
        }),
    );
}
