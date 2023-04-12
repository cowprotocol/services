use {
    super::SOLVER_NAME,
    crate::{
        domain::quote,
        infra,
        tests::{hex_address, setup},
    },
    itertools::Itertools,
    serde_json::json,
};

/// Test that the /quote endpoint behaves as expected.
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
        token_a_in_amount,
        token_b_out_amount,
        weth,
        interactions: uniswap_interactions,
        geth,
        solver_address,
        solver_secret_key,
        ..
    } = setup::blockchain::uniswap_a_b::setup().await;

    // Values for the auction.
    let sell_token = token_a.address();
    let buy_token = token_b.address();
    let sell_amount = token_a_in_amount;
    let buy_amount = token_b_out_amount;
    let gas_price = setup::blockchain::effective_gas_price(&web3).await;
    let now = infra::time::Now::Fake(chrono::Utc::now());
    let deadline = now.now() + chrono::Duration::seconds(2);
    let interactions = uniswap_interactions
        .iter()
        .map(|interaction| {
            json!({
                "kind": "custom",
                "internalize": false,
                "target": hex_address(interaction.address),
                "value": "0",
                "callData": format!("0x{}", hex::encode(&interaction.calldata)),
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
                "id": null,
                "tokens": {},
                "orders": [
                    {
                        "uid": "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                        "sellToken": hex_address(sell_token),
                        "buyToken": hex_address(buy_token),
                        "sellAmount": sell_amount.to_string(),
                        "buyAmount": "1",
                        "feeAmount": "0",
                        "kind": "sell",
                        "partiallyFillable": false,
                        "class": "market",
                    }
                ],
                "liquidity": [],
                "effectiveGasPrice": gas_price.to_string(),
                "deadline": deadline - quote::Deadline::time_buffer(),
            }),
            res: json!({
                "prices": {
                    hex_address(sell_token): buy_amount.to_string(),
                    hex_address(buy_token): sell_amount.to_string(),
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order":  "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
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

    // Call /quote.
    let result = client
        .quote(
            SOLVER_NAME,
            json!({
                "sellToken": hex_address(sell_token),
                "buyToken": hex_address(buy_token),
                "amount": sell_amount.to_string(),
                "kind": "sell",
                "effectiveGasPrice": gas_price.to_string(),
                "deadline": deadline,
            }),
        )
        .await;

    // Assert.
    assert!(result.is_object());
    assert_eq!(result.as_object().unwrap().len(), 2);
    assert!(result.get("amount").is_some());
    assert!(result.get("interactions").is_some());
    assert_eq!(
        result.get("amount").unwrap(),
        buy_amount.to_string().as_str()
    );

    let interactions = result.get("interactions").unwrap().as_array().unwrap();
    assert_eq!(interactions.len(), uniswap_interactions.len());
    for (actual, expected) in interactions.iter().zip(uniswap_interactions) {
        assert_eq!(
            actual.get("target").unwrap(),
            hex_address(expected.address).as_str()
        );
        assert_eq!(actual.get("value").unwrap(), "0");
        assert_eq!(
            actual.get("callData").unwrap(),
            &format!("0x{}", hex::encode(expected.calldata))
        );
    }
}
