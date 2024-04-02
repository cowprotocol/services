//! This test verifies that rounding is done in favour of the traders. This is
//! a weird detail that stems from the fact the UniswapV2-like pool swaps are
//! encoded as **swapTokensForExactTokens** (i.e. a "buy" order). The reason for
//! this is to make settlements less likely to revert (because buy swaps have
//! order fees as guaranteed "buffers", while sell swaps only have buffers if
//! they are already in the contract). The rounding is needed because the
//! settlement contract will round executed amounts in favour or the trader,
//! meaning that the clearing prices can cause the total buy amount to be a few
//! wei larger than the exact output that is encoded.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn test() {
    let engine = tests::SolverEngine::new(
        "naive",
        tests::Config::String(r#"risk-parameters = [0,0,0,0]"#.to_owned()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "9000000",
                    "buyAmount": "8500000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x000000000000000000000000000000000000000b",
                    "buyToken": "0x000000000000000000000000000000000000000a",
                    "sellAmount": "8500000",
                    "buyAmount": "8000001",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x000000000000000000000000000000000000000a": {
                            "balance": "1000000000000000000"
                        },
                        "0x000000000000000000000000000000000000000b": {
                            "balance": "1000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xffffffffffffffffffffffffffffffffffffffff",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
                    "gasEstimate": "110000"
                },
            ],
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
                    "0x000000000000000000000000000000000000000a": "996999",
                    "0x000000000000000000000000000000000000000b": "999999",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0101010101010101010101010101010101010101010101010101010101010101\
                                    0101010101010101010101010101010101010101\
                                    01010101",
                        "executedAmount": "9000000",
                    },
                    {
                        "kind": "fulfillment",
                        "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                    0202020202020202020202020202020202020202\
                                    02020202",
                        "executedAmount": "8000001",
                    },
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x000000000000000000000000000000000000000a",
                        "outputToken": "0x000000000000000000000000000000000000000b",
                        "inputAmount": "999999",
                        "outputAmount": "997000"
                    },
                ],
                "gas": 259417,
            }]
        }),
    );
}
