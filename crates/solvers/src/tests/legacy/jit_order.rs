//! Tests that solutions that use just-in-time liquidity orders get correctly
//! serialized.

use {
    crate::tests::{self, legacy, mock},
    serde_json::json,
};

#[tokio::test]
async fn test() {
    let legacy_solver = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: mock::http::RequestBody::Exact(json!({
            "amms": {},
            "metadata": {
                "auction_id": 1,
                "environment": "Ethereum / Mainnet",
                "gas_price": 15000000000.0,
                "native_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "run_id": null
            },
            "orders": {},
            "tokens": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "decimals": 18,
                    "normalize_priority": 1,
                    "accepted_for_internalization": false,
                    "internal_buffer": null,
                    "alias": null,
                    "external_price": null,
                }
            }
        })),
        res: json!({
            "orders": {},
            "prices": {},
            "amms": {},
            "foreign_liquidity_orders": [
                {
                    "order": {
                        "from": "0x1111111111111111111111111111111111111111",
                        "sellToken": "0x2222222222222222222222222222222222222222",
                        "buyToken": "0x3333333333333333333333333333333333333333",
                        "receiver": "0x4444444444444444444444444444444444444444",
                        "sellAmount": "100",
                        "buyAmount": "200",
                        "validTo": 1000,
                        "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                        "feeAmount": "0",
                        "kind": "sell",
                        "partiallyFillable": true,
                        "sellTokenBalance": "erc20",
                        "buyTokenBalance": "erc20",
                        "signingScheme": "eip712",
                        "signature": "0x\
                            0101010101010101010101010101010101010101010101010101010101010101\
                            0202020202020202020202020202020202020202020202020202020202020202\
                            03",
                        "interactions": {
                            "pre": [],
                            "post": []
                        }
                    },
                    "exec_sell_amount": "100",
                    "exec_buy_amount": "200",
                }
            ],
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("legacy", legacy::config(&legacy_solver.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z"
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {},
                "trades": [
                    {
                        "kind": "jit",
                        "order": {
                            "sellToken": "0x2222222222222222222222222222222222222222",
                            "buyToken": "0x3333333333333333333333333333333333333333",
                            "receiver": "0x4444444444444444444444444444444444444444",
                            "sellAmount": "100",
                            "buyAmount": "200",
                            "validTo": 1000,
                            "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                            "feeAmount": "0",
                            "kind": "sell",
                            "partiallyFillable": true,
                            "sellTokenBalance": "erc20",
                            "buyTokenBalance": "erc20",
                            "signingScheme": "eip712",
                            "signature": "0x\
                                0101010101010101010101010101010101010101010101010101010101010101\
                                0202020202020202020202020202020202020202020202020202020202020202\
                                03",
                        },
                        "executedAmount": "100",
                    }
                ],
                "interactions": [],
            }]
        }),
    );
}
