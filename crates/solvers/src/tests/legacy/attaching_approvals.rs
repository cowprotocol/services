//! Tests that approvals get attached to the first non-internalizable
//! interaction.

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
                "auction_id": 1234,
                "environment": "Ethereum / Mainnet",
                "gas_price": 15000000000.0,
                "native_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "run_id": null,
            },
            "orders": {},
            "tokens": {
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "accepted_for_internalization": true,
                    "internal_buffer": "0",
                    "decimals": 18,
                    "alias": null,
                    "external_price": null,
                    "normalize_priority": 1,
                },
                "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab": {
                    "accepted_for_internalization": true,
                    "internal_buffer": "0",
                    "decimals": null,
                    "alias": null,
                    "external_price": null,
                    "normalize_priority": 0,
                }
            }
        })),
        res: json!({
            "orders": {},
            "prices": {},
            "interaction_data": [
                {
                    "target": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                    "value": "0",
                    "call_data": "0x",
                    "inputs": [],
                    "outputs": [],
                    "exec_plan": {
                        "sequence": 0,
                        "position": 2,
                        "internal": false,
                    },
                },
                {
                    "target": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                    "value": "0",
                    "call_data": "0x",
                    "inputs": [],
                    "outputs": [],
                    "exec_plan": {
                        "sequence": 0,
                        "position": 1,
                        "internal": true,
                    },
                },
            ],
            "approvals": [
                {
                    "token": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "spender": "0x1111111111111111111111111111111111111111",
                    "amount": "1",
                },
                {
                    "token": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "spender": "0x2222222222222222222222222222222222222222",
                    "amount": "2",
                },
            ],
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("legacy", legacy::config(&legacy_solver.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1234",
            "tokens": {
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
                    "availableBalance": "0",
                    "trusted": true
                }
            },
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
                "trades": [],
                "preInteractions": [],
                "interactions": [
                    {
                        "target": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "value": "0",
                        "callData": "0x",
                        "inputs": [],
                        "outputs": [],
                        "internalize": true,
                        "allowances": [],
                        "kind": "custom",
                    },
                    {
                        "target": "0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab",
                        "value": "0",
                        "callData": "0x",
                        "inputs": [],
                        "outputs": [],
                        "internalize": false,
                        "allowances": [
                            {
                                "token": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                                "spender": "0x1111111111111111111111111111111111111111",
                                "amount": "1",
                            },
                            {
                                "token": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                                "spender": "0x2222222222222222222222222222222222222222",
                                "amount": "2",
                            },
                        ],
                        "kind": "custom",
                    },
                ],
                "postInteractions": [],
            }]
        }),
    );
}
