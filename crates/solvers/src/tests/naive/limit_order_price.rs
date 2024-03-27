//! This test verifies that the limit order's limit price is respected after
//! surplus fees are taken from the order.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn test() {
    let engine = tests::SolverEngine::new(
        "naive",
        tests::Config::String(r#"risk-parameters = [0,0,0,0]"#.to_owned()),
    )
    .await;

    // pool price is 1:1 with 1000 units of each token
    // order sells 1 unit for 0.9 units
    // leaving us with 0.1 units of surplus (modulo price impact)
    // order is estimated to cost ~200000 units of gas to execute
    // with a native price of 1:1 we should find a solution as
    // long as gas price < 500000000000
    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {
                    "availableBalance": "0",
                    "trusted": false,
                    "referencePrice": "1000000000000000000",
                },
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "availableBalance": "0",
                    "trusted": false,
                    "referencePrice": "1000000000000000000",
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "buyToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "900000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "limit",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {
                            "balance": "1000000000000000000000"
                        },
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                            "balance": "1000000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0x0000000000000000000000000000000000000000",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
                    "gasEstimate": "110000"
                },
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await;

    // solution has fee + executedAmount == order.sellAmount
    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "gas": 204391,
                "id": 0,
                "interactions": [{
                    "id": "0",
                    "inputAmount": "1000000000000000000",
                    "inputToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "internalize": false,
                    "kind": "liquidity",
                    "outputAmount": "996006981039903216",
                    "outputToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                }],
                "prices": {
                    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "996006981039903216",
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1000000000000000000",
                },
                "score": {
                    "kind": "riskAdjusted",
                    "successProbability": 0.5,
                },
                "trades": [{
                    "executedAmount": "996274135000000000",
                    "fee": "3725865000000000",
                    "kind": "fulfillment",
                    "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a",
                }]
            }]
        }),
    );
}
