//! Test that the asset flow verification behaves as expected for market orders.
//! See [`competition::solution::settlement::Verified`].

use {
    crate::{
        domain::{
            competition::{self, auction},
            eth,
        },
        infra,
        tests::{self, cases::SOLVER_NAME, hex_address, setup},
    },
    itertools::Itertools,
    serde_json::json,
};

#[derive(Debug, Default)]
struct Flow {
    inputs: Vec<eth::Asset>,
    outputs: Vec<eth::Asset>,
}

struct TestCase {
    flow: fn(sell: eth::Asset, buy: eth::Asset, idx: usize) -> Flow,
    valid: bool,
}

#[tokio::test]
#[ignore]
async fn test() {
    crate::boundary::initialize_tracing("driver=trace");

    let cases = [
        TestCase {
            flow: |sell, buy, idx| match idx {
                0 => Flow {
                    inputs: vec![sell],
                    outputs: vec![buy],
                },
                1 => Default::default(),
                _ => unreachable!(),
            },
            valid: true,
        },
        TestCase {
            flow: |sell, buy, idx| match idx {
                // The inputs and outputs are spread across different interactions.
                0 => Flow {
                    inputs: vec![sell],
                    outputs: vec![],
                },
                1 => Flow {
                    inputs: vec![],
                    outputs: vec![buy],
                },
                _ => unreachable!(),
            },
            valid: true,
        },
        TestCase {
            flow: |sell, buy, idx| match idx {
                0 => Flow {
                    // The interaction input is higher than the output, meaning that
                    // the settlement takes money out of the contract, which is illegal.
                    inputs: vec![eth::Asset {
                        token: sell.token,
                        amount: sell.amount + eth::U256::from(12),
                    }],
                    outputs: vec![buy],
                },
                1 => Default::default(),
                _ => unreachable!(),
            },
            valid: false,
        },
        TestCase {
            flow: |sell, buy, idx| match idx {
                0 => Flow {
                    // The interaction output is higher than the input, leaving money in the
                    // settlement contract. This is OK!
                    inputs: vec![sell],
                    outputs: vec![eth::Asset {
                        token: buy.token,
                        amount: buy.amount + eth::U256::from(12),
                    }],
                },
                1 => Default::default(),
                _ => unreachable!(),
            },
            valid: true,
        },
        TestCase {
            flow: |sell, buy, idx| match idx {
                // The inputs and outputs are spread across different interactions.
                0 => Flow {
                    inputs: vec![eth::Asset {
                        token: sell.token,
                        amount: sell.amount - 20,
                    }],
                    outputs: vec![eth::Asset {
                        token: buy.token,
                        amount: buy.amount - 30,
                    }],
                },
                1 => Flow {
                    inputs: vec![eth::Asset {
                        token: sell.token,
                        amount: 20.into(),
                    }],
                    outputs: vec![eth::Asset {
                        token: buy.token,
                        amount: 30.into(),
                    }],
                },
                _ => unreachable!(),
            },
            valid: true,
        },
        TestCase {
            flow: |sell, buy, idx| match idx {
                // The inputs and outputs are spread across different interactions.
                0 => Flow {
                    inputs: vec![eth::Asset {
                        token: sell.token,
                        amount: sell.amount - 20,
                    }],
                    outputs: vec![eth::Asset {
                        token: buy.token,
                        amount: buy.amount - 30,
                    }],
                },
                1 => Flow {
                    // More money coming out of the contract - illegal!
                    inputs: vec![eth::Asset {
                        token: sell.token,
                        amount: 30.into(),
                    }],
                    outputs: vec![eth::Asset {
                        token: buy.token,
                        amount: 30.into(),
                    }],
                },
                _ => unreachable!(),
            },
            valid: false,
        },
        TestCase {
            flow: |sell, buy, idx| match idx {
                // The inputs and outputs are spread across different interactions.
                0 => Flow {
                    inputs: vec![eth::Asset {
                        token: sell.token,
                        amount: sell.amount - 20,
                    }],
                    outputs: vec![eth::Asset {
                        token: buy.token,
                        amount: buy.amount - 30,
                    }],
                },
                1 => Flow {
                    // Less money taken out of the contract - OK!
                    inputs: vec![eth::Asset {
                        token: sell.token,
                        amount: 10.into(),
                    }],
                    outputs: vec![eth::Asset {
                        token: buy.token,
                        amount: 30.into(),
                    }],
                },
                _ => unreachable!(),
            },
            valid: true,
        },
    ];

    for TestCase { flow, valid } in cases {
        // Set up the uniswap swap.
        let setup::blockchain::Uniswap {
            web3,
            settlement,
            token_a,
            token_b,
            admin,
            domain_separator,
            user_fee,
            token_a_in_amount,
            token_b_out_amount,
            weth,
            admin_secret_key,
            interactions,
            solver_address,
            geth,
            solver_secret_key,
        } = setup::blockchain::uniswap::setup().await;

        // Values for the auction.
        let sell_token = token_a.address();
        let buy_token = token_b.address();
        let sell_amount = token_a_in_amount;
        let buy_amount = token_b_out_amount;
        let valid_to = u32::MAX;
        let boundary = tests::boundary::Order {
            sell_token,
            buy_token,
            sell_amount,
            buy_amount,
            valid_to,
            user_fee,
            side: competition::order::Side::Sell,
            secret_key: admin_secret_key,
            domain_separator,
            owner: admin,
            partially_fillable: false,
        };
        let gas_price = web3.eth().gas_price().await.unwrap().to_string();
        let now = infra::time::Now::Fake(chrono::Utc::now());
        let deadline = now.now() + chrono::Duration::days(30);
        let interactions = interactions
            .into_iter()
            .enumerate()
            .map(|(idx, interaction)| {
                let flow = flow(
                    eth::Asset {
                        token: sell_token.into(),
                        amount: sell_amount,
                    },
                    eth::Asset {
                        token: buy_token.into(),
                        amount: buy_amount,
                    },
                    idx,
                );
                json!({
                    "kind": "custom",
                    "internalize": false,
                    "target": hex_address(interaction.address),
                    "value": "0",
                    "callData": format!("0x{}", hex::encode(interaction.calldata)),
                    "allowances": [],
                    "inputs": flow.inputs.iter().map(|asset| json!({
                        "token": hex_address(asset.token.into()),
                        "amount": asset.amount.to_string(),
                    })).collect_vec(),
                    "outputs": flow.outputs.iter().map(|asset| json!({
                        "token": hex_address(asset.token.into()),
                        "amount": asset.amount.to_string(),
                    })).collect_vec(),
                })
            })
            .collect_vec();

        // Set up the solver.
        let solver = setup::solver::setup(setup::solver::Config {
            name: SOLVER_NAME.to_owned(),
            absolute_slippage: "0".to_owned(),
            relative_slippage: "0.0".to_owned(),
            address: hex_address(solver_address),
            private_key: format!("0x{}", solver_secret_key.display_secret()),
            solve: vec![setup::solver::Solve {
                req: json!({
                    "id": "1",
                    "tokens": {
                        hex_address(sell_token): {
                            "decimals": null,
                            "symbol": null,
                            "referencePrice": "1",
                            "availableBalance": "0",
                            "trusted": false,
                        },
                        hex_address(buy_token): {
                            "decimals": null,
                            "symbol": null,
                            "referencePrice": "2",
                            "availableBalance": "0",
                            "trusted": false,
                        }
                    },
                    "orders": [
                        {
                            "uid": boundary.uid(),
                            "sellToken": hex_address(sell_token),
                            "buyToken": hex_address(buy_token),
                            "sellAmount": sell_amount.to_string(),
                            "buyAmount": buy_amount.to_string(),
                            "feeAmount": "0",
                            "kind": "sell",
                            "partiallyFillable": false,
                            "class": "market",
                            "reward": 0.1,
                        }
                    ],
                    "liquidity": [],
                    "effectiveGasPrice": gas_price,
                    "deadline": deadline - auction::Deadline::time_buffer(),
                }),
                res: json!({
                    "prices": {
                        hex_address(sell_token): buy_amount.to_string(),
                        hex_address(buy_token): sell_amount.to_string(),
                    },
                    "trades": [
                        {
                            "kind": "fulfillment",
                            "order": boundary.uid(),
                            "executedAmount": sell_amount.to_string(),
                        }
                    ],
                    "interactions": interactions
                }),
            }],
        })
        .await;

        // Set up the driver.
        let client = setup::driver::setup(setup::driver::Config {
            now,
            file: setup::driver::ConfigFile::Create {
                solvers: vec![solver],
                contracts: infra::config::file::ContractsConfig {
                    gp_v2_settlement: Some(settlement.address()),
                    weth: Some(weth.address()),
                },
            },
            geth: &geth,
        })
        .await;

        // Call /solve.
        let (status, result) = client
        .solve(
            SOLVER_NAME,
            json!({
                "id": 1,
                "tokens": [
                    {
                        "address": hex_address(sell_token),
                        "price": "1",
                        "trusted": false,
                    },
                    {
                        "address": hex_address(buy_token),
                        "price": "2",
                        "trusted": false,
                    }
                ],
                "orders": [
                    {
                        "uid": boundary.uid(),
                        "sellToken": hex_address(sell_token),
                        "buyToken": hex_address(buy_token),
                        "sellAmount": sell_amount.to_string(),
                        "buyAmount": buy_amount.to_string(),
                        "solverFee": "0",
                        "userFee": user_fee.to_string(),
                        "validTo": valid_to,
                        "kind": "sell",
                        "owner": hex_address(admin),
                        "partiallyFillable": false,
                        "executed": "0",
                        "preInteractions": [],
                        "class": "market",
                        "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                        "reward": 0.1,
                        "signingScheme": "eip712",
                        "signature": format!("0x{}", hex::encode(boundary.signature()))
                    }
                ],
                "deadline": deadline,
            }),
        )
        .await;

        // Assert.
        if valid {
            assert_eq!(status, hyper::StatusCode::OK);
            assert!(result.is_object());
            assert_eq!(result.as_object().unwrap().len(), 2);
            assert!(result.get("id").is_some());
            assert!(result.get("score").is_some());
        } else {
            assert_eq!(status, hyper::StatusCode::BAD_REQUEST);
            assert!(result.is_object());
            assert_eq!(result.as_object().unwrap().len(), 2);
            assert!(result.get("kind").is_some());
            assert!(result.get("description").is_some());
            let kind = result.get("kind").unwrap().as_str().unwrap();
            assert_eq!(kind, "InvalidAssetFlow");
        }
    }
}
