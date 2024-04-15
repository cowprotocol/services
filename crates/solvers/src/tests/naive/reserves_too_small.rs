//! This test verifies that it filters out large orders when pool reserves are
//! too small to be able to handle them.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn test() {
    let engine = tests::SolverEngine::new("naive", tests::Config::None).await;

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
                    "sellAmount": "70145218378783248142575",
                    "fullSellAmount": "70145218378783248142575",
                    "buyAmount": "70123226323",
                    "fullBuyAmount": "70123226323",
                    "feePolicies": [],
                    "validTo": 0,
                    "kind": "sell",
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "partiallyFillable": false,
                    "preInteractions": [],
                    "postInteractions": [],
                    "class": "market",
                    "signingScheme": "presign",
                    "signature": "0x",
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "900000000000000",
                    "fullSellAmount": "900000000000000",
                    "buyAmount": "100",
                    "fullBuyAmount": "100",
                    "feePolicies": [],
                    "validTo": 0,
                    "kind": "sell",
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "partiallyFillable": false,
                    "preInteractions": [],
                    "postInteractions": [],
                    "class": "market",
                    "signingScheme": "presign",
                    "signature": "0x",
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x000000000000000000000000000000000000000a": {
                            "balance": "25000075"
                        },
                        "0x000000000000000000000000000000000000000b": {
                            "balance": "2500007500"
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
                    "0x000000000000000000000000000000000000000a": "2500007430",
                    "0x000000000000000000000000000000000000000b": "900000000000000",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                    0202020202020202020202020202020202020202\
                                    02020202",
                        "executedAmount": "900000000000000",
                    },
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x000000000000000000000000000000000000000a",
                        "outputToken": "0x000000000000000000000000000000000000000b",
                        "inputAmount": "900000000000000",
                        "outputAmount": "2500007430"
                    },
                ],
                "gas": 204391,
            }]
        }),
    );
}
