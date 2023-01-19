use {
    crate::{
        domain::competition,
        infra::{self, config::cli},
        tests::{hex_address, setup},
    },
    itertools::Itertools,
    serde_json::json,
};

/// Test that the /quote endpoint behaves as expected.
#[ignore]
#[tokio::test]
async fn test() {
    // Set up the uniswap swap.
    let setup::blockchain::Uniswap {
        web3,
        settlement,
        token_a,
        token_b,
        admin,
        token_a_in_amount,
        token_b_out_amount,
        weth,
        interactions: uniswap_interactions,
        ..
    } = setup::blockchain::uniswap::setup().await;

    // Values for the auction.
    let sell_token = token_a.address();
    let buy_token = token_b.address();
    let sell_amount = token_a_in_amount;
    let buy_amount = token_b_out_amount;
    let gas_price = web3.eth().gas_price().await.unwrap().to_string();
    let now = infra::time::Now::Fake(chrono::Utc::now());
    let deadline = now.now()
        + chrono::Duration::milliseconds(setup::driver::QUOTE_TIMEOUT_MS.try_into().unwrap());
    let interactions = uniswap_interactions
        .iter()
        .map(|(address, interaction)| {
            json!({
                "kind": "custom",
                "internalize": false,
                "target": hex_address(address.to_owned()),
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
        name: "test1".to_owned(),
        absolute_slippage: "0".to_owned(),
        relative_slippage: "0.0".to_owned(),
        address: hex_address(admin),
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
                        "reward": competition::quote::FAKE_AUCTION_REWARD,
                    }
                ],
                "liquidity": [],
                "effectiveGasPrice": gas_price,
                "deadline": deadline,
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
        contracts: cli::ContractAddresses {
            gp_v2_settlement: Some(settlement.address()),
            weth: Some(weth.address()),
        },
        solvers: setup::driver::SolversConfig::CreateConfigFile(vec![solver]),
    })
    .await;

    // Call /quote.
    let result = client
        .quote(
            "test1",
            json!({
                "sellToken": hex_address(sell_token),
                "buyToken": hex_address(buy_token),
                "amount": sell_amount.to_string(),
                "kind": "sell",
                "effectiveGasPrice": gas_price,
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
    for (interaction, (target, call_data)) in interactions.iter().zip(uniswap_interactions) {
        assert_eq!(
            interaction.get("target").unwrap(),
            hex_address(target).as_str()
        );
        assert_eq!(interaction.get("value").unwrap(), "0");
        assert_eq!(
            interaction.get("callData").unwrap(),
            &format!("0x{}", hex::encode(call_data))
        );
    }
}
