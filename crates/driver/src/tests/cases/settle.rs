use {
    super::SOLVER_NAME,
    crate::{
        domain::competition::{self, auction},
        infra,
        tests::{self, hex_address, setup},
    },
    itertools::Itertools,
    serde_json::json,
    web3::types::TransactionId,
};

/// Test that the /settle endpoint behaves as expected.
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
    };
    let now = infra::time::Now::Fake(chrono::Utc::now());
    let deadline = now.now() + chrono::Duration::days(30);
    let interactions = interactions
        .into_iter()
        .map(|(address, interaction)| {
            json!({
                "kind": "custom",
                "internalize": false,
                "target": hex_address(address),
                "value": "0",
                "callData": format!("0x{}", hex::encode(interaction)),
                "allowances": [],
                "inputs": [],
                "outputs": [],
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
                "effectiveGasPrice": "273805601",
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
    let (status, solution) = client
        .solve(
            SOLVER_NAME,
            json!({
                "id": 1,
                "prices": {
                    hex_address(sell_token): "1",
                    hex_address(buy_token): "2",
                },
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

    let solution_id = solution.get("id").unwrap().as_str().unwrap();
    let block_number = web3.eth().block_number().await.unwrap();
    let old_balance = web3.eth().balance(solver_address, None).await.unwrap();
    let old_token_a = token_a.balance_of(admin).call().await.unwrap();
    let old_token_b = token_b.balance_of(admin).call().await.unwrap();

    setup::blockchain::wait_for(&web3, client.settle(SOLVER_NAME, solution_id)).await;

    // Assert.
    assert_eq!(status, hyper::StatusCode::OK);
    let new_balance = web3.eth().balance(solver_address, None).await.unwrap();
    let new_token_a = token_a.balance_of(admin).call().await.unwrap();
    let new_token_b = token_b.balance_of(admin).call().await.unwrap();
    // ETH balance is lower due to transaction fees.
    assert!(new_balance < old_balance);
    // The balance of the trader changes according to the swap.
    assert_eq!(new_token_a, old_token_a - token_a_in_amount - user_fee);
    assert_eq!(new_token_b, old_token_b + token_b_out_amount);

    // Check that the solution ID is included in the settlement.
    let tx = web3
        .eth()
        .transaction(TransactionId::Block((block_number + 1).into(), 0.into()))
        .await
        .unwrap()
        .unwrap();
    let input = tx.input.0;
    let len = input.len();
    let tx_solution_id = u64::from_be_bytes((&input[len - 8..]).try_into().unwrap());
    assert_eq!(tx_solution_id.to_string(), solution_id);
}
