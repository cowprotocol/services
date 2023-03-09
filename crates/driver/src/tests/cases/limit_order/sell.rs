//! Test that a valid sell limit order is settled correctly.

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

#[tokio::test]
#[ignore]
async fn test() {
    crate::boundary::initialize_tracing("driver=trace");
    // Set up the uniswap swap.
    let setup::blockchain::Uniswap {
        web3,
        settlement,
        token_a,
        token_b,
        admin,
        domain_separator,
        token_a_in_amount,
        token_b_out_amount,
        weth,
        admin_secret_key,
        interactions,
        solver_address,
        geth,
        solver_secret_key,
        ..
    } = setup::blockchain::uniswap::setup().await;

    // Values for the auction.
    let sell_token = token_a.address();
    let buy_token = token_b.address();
    let surplus_fee = eth::U256::from(10);
    let sell_amount = token_a_in_amount + surplus_fee;
    let buy_amount = token_b_out_amount;
    let valid_to = u32::MAX;
    let boundary = tests::boundary::Order {
        sell_token,
        buy_token,
        sell_amount,
        buy_amount,
        valid_to,
        user_fee: 0.into(),
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
                        "sellAmount": (sell_amount - surplus_fee).to_string(),
                        "buyAmount": buy_amount.to_string(),
                        "feeAmount": "0",
                        "kind": "sell",
                        "partiallyFillable": false,
                        "class": "limit",
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
                    hex_address(buy_token): (sell_amount - surplus_fee).to_string(),
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
    let (status, solution) = client
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
                        "userFee": "0",
                        "validTo": valid_to,
                        "kind": "sell",
                        "owner": hex_address(admin),
                        "partiallyFillable": false,
                        "executed": "0",
                        "preInteractions": [],
                        "class": "limit",
                        "surplusFee": surplus_fee.to_string(),
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

    // Assert that the solution is valid.
    assert_eq!(status, hyper::StatusCode::OK);
    assert!(solution.is_object());
    assert_eq!(solution.as_object().unwrap().len(), 2);
    assert!(solution.get("id").is_some());
    assert!(solution.get("score").is_some());
    let score = solution.get("score").unwrap().as_f64().unwrap();
    approx::assert_relative_eq!(score, -58130959128924.0, max_relative = 0.01);

    let old_token_a = token_a.balance_of(admin).call().await.unwrap();
    let old_token_b = token_b.balance_of(admin).call().await.unwrap();

    // Call /settle.
    setup::blockchain::wait_for(
        &web3,
        client.settle(SOLVER_NAME, solution.get("id").unwrap().as_str().unwrap()),
    )
    .await;

    // Assert that the settlement is valid.
    let new_token_a = token_a.balance_of(admin).call().await.unwrap();
    let new_token_b = token_b.balance_of(admin).call().await.unwrap();
    // The balance of the trader changes according to the swap.
    assert_eq!(new_token_a, old_token_a - token_a_in_amount - surplus_fee);
    assert_eq!(new_token_b, old_token_b + token_b_out_amount);
}
