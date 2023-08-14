//! Simple test case that verifies that the baseline solver can settle a buy
//! order directly with a Uniswap V2 pool, even if there is a large token
//! decimals difference, which leads to weird rounding behaviour.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn test() {
    let engine = tests::SolverEngine::new(
        "baseline",
        tests::Config::File("config/example.baseline.toml".into()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1412206645170290748",
                    "trusted": true
                },
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48": {
                    "decimals": 6,
                    "symbol": "USDC",
                    "referencePrice": "543222446200924874026413848",
                    "availableBalance": "556450389",
                    "trusted": true
                }
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                    "buyToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "sellAmount": "2000000000",
                    "buyAmount": "1000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                }
            ],
            "liquidity": [
                {
                    "kind": "constantproduct",
                    "tokens": {
                        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48": {
                            "balance": "30493445841295"
                        },
                        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                            "balance": "16551311935742077745684"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc",
                    "gasEstimate": "110000"
                }
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    // Note that the interaction executes slightly more than the buy order's
    // amount. This is inevitable because of rounding - if we sold 1 less wei
    // of the input token, we would not be able to buy enough to cover the buy
    // order, the difference stays in the settlement contract.
    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1848013595",
                    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "1000000000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000"
                    }
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                        "outputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "inputAmount": "1848013595",
                        "outputAmount": "1000000000428620302"
                    }
                ]
            }]
        }),
    );
}
