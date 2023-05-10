//! Test that verifies solver balance are verified for solutions.

use {
    super::SOLVER_NAME,
    crate::{
        domain::{
            competition::{self, auction},
            eth,
        },
        infra,
        tests::{self, hex_address, setup},
    },
    itertools::Itertools,
    serde_json::json,
};

/// Test that the `/solve` request errors when solver balance is low.
#[tokio::test]
#[ignore]
async fn test() {
    crate::boundary::initialize_tracing("driver=trace");

    // Set up the uniswap swap.
    let setup::blockchain::uniswap_a_b::Uniswap {
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
    } = setup::blockchain::uniswap_a_b::setup().await;

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
    let now = infra::time::Now::Fake(chrono::Utc::now());
    let deadline = now.now() + chrono::Duration::days(30);
    let interactions = interactions
        .into_iter()
        .map(|interaction| {
            json!({
                "kind": "custom",
                "internalize": false,
                "target": hex_address(interaction.address),
                "value": "0",
                "callData": format!("0x{}", hex::encode(interaction.calldata)),
                "allowances": [],
                "inputs": interaction.inputs.iter().map(|input| {
                    json!({
                        "token": hex_address(input.token.into()),
                        "amount": input.amount.to_string(),
                    })
                }).collect_vec(),
                "outputs": interaction.outputs.iter().map(|output| {
                    json!({
                        "token": hex_address(output.token.into()),
                        "amount": output.amount.to_string(),
                    })
                }).collect_vec(),
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
                    }
                ],
                "liquidity": [],
                "effectiveGasPrice": "247548525",
                "deadline": deadline - auction::Deadline::time_buffer(),
            }),
            res: json!({
                "solutions": [
                    {
                        "id": 0,
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
                    }
                ]
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

    // Transfer out all of the solver's funds.
    setup::blockchain::wait_for(&web3, {
        // An empirically determined balance (from the `settle` test) that is
        // more than enough to actually execute the settlement on-chain **but**
        // too small to account for gas spikes.
        let leftover = eth::U256::from(100_000_000_000_000_u128);

        let balance = web3.eth().balance(solver_address, None).await.unwrap();
        let gas_price = web3.eth().gas_price().await.unwrap();

        let tx = web3
            .accounts()
            .sign_transaction(
                web3::types::TransactionParameters {
                    to: Some(Default::default()),
                    value: balance - leftover - gas_price * 21000,
                    gas: 21000.into(),
                    gas_price: Some(gas_price),
                    ..Default::default()
                },
                &solver_secret_key,
            )
            .await
            .unwrap();

        let web3 = web3.clone();
        async move {
            web3.eth()
                .send_raw_transaction(tx.raw_transaction)
                .await
                .unwrap()
        }
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
                        "postInteractions": [],
                        "class": "market",
                        "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                        "signingScheme": "eip712",
                        "signature": format!("0x{}", hex::encode(boundary.signature()))
                    }
                ],
                "deadline": deadline,
            }),
        )
        .await;

    // Assert.
    assert_eq!(status, hyper::StatusCode::BAD_REQUEST);
    assert!(result.is_object());
    assert_eq!(result.as_object().unwrap().len(), 2);
    assert!(result.get("kind").is_some());
    assert!(result.get("description").is_some());
    let kind = result.get("kind").unwrap().as_str().unwrap();
    // TODO When we add metrics, assert that an insufficient balance error is
    // traced.
    assert_eq!(kind, "SolutionNotFound");
}
