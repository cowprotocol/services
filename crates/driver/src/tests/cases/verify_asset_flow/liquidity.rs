//! Test that the asset flow verification behaves as expected for liquidity
//! orders. See [`competition::solution::settlement::Verified`].

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

#[derive(Debug)]
struct LiquidityOrder {
    sell: eth::Asset,
    buy: eth::Asset,
    side: competition::order::Side,
    executed: eth::U256,
    partially_fillable: bool,
}

struct TestCase {
    valid: bool,
    liquidity_orders: fn(sell_token: eth::H160, buy_token: eth::H160) -> Vec<LiquidityOrder>,
}

#[tokio::test]
#[ignore]
async fn test() {
    crate::boundary::initialize_tracing("debug,hyper=warn");

    // The test setup below makes a uniswap interaction for a certain buy and sell
    // amount. The market order included in the auction is 50 sell tokens and 50
    // buy tokens less than the interaction. For the asset flow to be valid, the
    // liquidity orders specified in these test cases are supposed to make up
    // for that difference.
    let test_cases = [
        TestCase {
            // Single sell order which is not partially fillable.
            valid: true,
            liquidity_orders: |sell_token: eth::H160, buy_token: eth::H160| {
                vec![LiquidityOrder {
                    sell: eth::Asset {
                        amount: 50.into(),
                        token: sell_token.into(),
                    },
                    buy: eth::Asset {
                        amount: 50.into(),
                        token: buy_token.into(),
                    },
                    side: competition::order::Side::Sell,
                    executed: 50.into(),
                    partially_fillable: false,
                }]
            },
        },
        TestCase {
            // Single sell order which is partially fillable.
            valid: true,
            liquidity_orders: |sell_token: eth::H160, buy_token: eth::H160| {
                vec![LiquidityOrder {
                    sell: eth::Asset {
                        amount: 100.into(),
                        token: sell_token.into(),
                    },
                    buy: eth::Asset {
                        amount: 100.into(),
                        token: buy_token.into(),
                    },
                    side: competition::order::Side::Sell,
                    executed: 50.into(),
                    partially_fillable: true,
                }]
            },
        },
        TestCase {
            // Single buy order which is not partially fillable.
            valid: true,
            liquidity_orders: |sell_token: eth::H160, buy_token: eth::H160| {
                vec![LiquidityOrder {
                    sell: eth::Asset {
                        amount: 50.into(),
                        token: sell_token.into(),
                    },
                    buy: eth::Asset {
                        amount: 50.into(),
                        token: buy_token.into(),
                    },
                    side: competition::order::Side::Buy,
                    executed: 50.into(),
                    partially_fillable: false,
                }]
            },
        },
        TestCase {
            // Single buy order which is partially fillable.
            valid: true,
            liquidity_orders: |sell_token: eth::H160, buy_token: eth::H160| {
                vec![LiquidityOrder {
                    sell: eth::Asset {
                        amount: 100.into(),
                        token: sell_token.into(),
                    },
                    buy: eth::Asset {
                        amount: 100.into(),
                        token: buy_token.into(),
                    },
                    side: competition::order::Side::Buy,
                    executed: 50.into(),
                    partially_fillable: true,
                }]
            },
        },
        TestCase {
            // Partially fillable sell and buy order.
            valid: true,
            liquidity_orders: |sell_token: eth::H160, buy_token: eth::H160| {
                vec![
                    // This order ends up selling 40 and buying 30.
                    LiquidityOrder {
                        sell: eth::Asset {
                            amount: 80.into(),
                            token: sell_token.into(),
                        },
                        buy: eth::Asset {
                            amount: 60.into(),
                            token: buy_token.into(),
                        },
                        side: competition::order::Side::Buy,
                        executed: 30.into(),
                        partially_fillable: true,
                    },
                    // This order ends up selling 10 and buying 20.
                    LiquidityOrder {
                        sell: eth::Asset {
                            amount: 100.into(),
                            token: sell_token.into(),
                        },
                        buy: eth::Asset {
                            amount: 200.into(),
                            token: buy_token.into(),
                        },
                        side: competition::order::Side::Sell,
                        executed: 10.into(),
                        partially_fillable: true,
                    },
                ]
            },
        },
        TestCase {
            // Single order which attempts to buy too much.
            valid: false,
            liquidity_orders: |sell_token: eth::H160, buy_token: eth::H160| {
                vec![LiquidityOrder {
                    sell: eth::Asset {
                        amount: 10.into(),
                        token: sell_token.into(),
                    },
                    buy: eth::Asset {
                        amount: 200.into(),
                        token: buy_token.into(),
                    },
                    side: competition::order::Side::Buy,
                    executed: 100.into(),
                    partially_fillable: true,
                }]
            },
        },
        TestCase {
            // Two orders which attempt to buy too much.
            valid: false,
            liquidity_orders: |sell_token: eth::H160, buy_token: eth::H160| {
                vec![
                    LiquidityOrder {
                        sell: eth::Asset {
                            amount: 15.into(),
                            token: sell_token.into(),
                        },
                        buy: eth::Asset {
                            amount: 10.into(),
                            token: buy_token.into(),
                        },
                        side: competition::order::Side::Sell,
                        executed: 5.into(),
                        partially_fillable: true,
                    },
                    LiquidityOrder {
                        sell: eth::Asset {
                            amount: 10.into(),
                            token: sell_token.into(),
                        },
                        buy: eth::Asset {
                            amount: 200.into(),
                            token: buy_token.into(),
                        },
                        side: competition::order::Side::Buy,
                        executed: 100.into(),
                        partially_fillable: true,
                    },
                ]
            },
        },
    ];

    for TestCase {
        valid,
        liquidity_orders,
    } in test_cases
    {
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
        let sell_amount = token_a_in_amount - eth::U256::from(50);
        let buy_amount = token_b_out_amount - eth::U256::from(50);
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
            .map(|interaction| {
                json!({
                    "kind": "custom",
                    "internalize": false,
                    "target": hex_address(interaction.address),
                    "value": "0",
                    "callData": format!("0x{}", hex::encode(interaction.calldata)),
                    "allowances": [],
                    "inputs": interaction.inputs.iter().map(|asset| json!({
                        "token": hex_address(asset.token.into()),
                        "amount": asset.amount.to_string(),
                    })).collect_vec(),
                    "outputs": interaction.outputs.iter().map(|asset| json!({
                        "token": hex_address(asset.token.into()),
                        "amount": asset.amount.to_string(),
                    })).collect_vec(),
                })
            })
            .collect_vec();

        let boundary_liquidity_order = |order: &LiquidityOrder| tests::boundary::Order {
            sell_token: order.sell.token.into(),
            buy_token: order.buy.token.into(),
            sell_amount: order.sell.amount,
            buy_amount: order.buy.amount,
            valid_to,
            user_fee: 0.into(),
            side: order.side,
            secret_key: admin_secret_key,
            domain_separator,
            owner: admin,
            partially_fillable: order.partially_fillable,
        };

        // Set up the solver.
        let mut orders = vec![json!({
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
        })];
        for liquidity_order in liquidity_orders(sell_token, buy_token) {
            orders.push({
                json!({
                    "uid": boundary_liquidity_order(&liquidity_order).uid(),
                    "sellToken": hex_address(liquidity_order.sell.token.into()),
                    "buyToken": hex_address(liquidity_order.buy.token.into()),
                    "sellAmount": liquidity_order.sell.amount.to_string(),
                    "buyAmount": liquidity_order.buy.amount.to_string(),
                    "feeAmount": "0",
                    "kind": match liquidity_order.side {
                        competition::order::Side::Sell => "sell",
                        competition::order::Side::Buy => "buy",
                    },
                    "partiallyFillable": liquidity_order.partially_fillable,
                    "class": "liquidity",
                    "reward": 0.1,
                })
            });
        }
        let mut trades = vec![json!({
            "kind": "fulfillment",
            "order": boundary.uid(),
            "executedAmount": sell_amount.to_string(),
        })];
        for liquidity_order in liquidity_orders(sell_token, buy_token) {
            trades.push({
                json!({
                    "kind": "fulfillment",
                    "order": boundary_liquidity_order(&liquidity_order).uid(),
                    "executedAmount": liquidity_order.executed.to_string(),
                })
            });
        }
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
                    "orders": orders,
                    "liquidity": [],
                    "effectiveGasPrice": gas_price,
                    "deadline": deadline - auction::Deadline::time_buffer(),
                }),
                res: json!({
                    "prices": {
                        hex_address(sell_token): buy_amount.to_string(),
                        hex_address(buy_token): sell_amount.to_string(),
                    },
                    "trades": trades,
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
        let mut orders = vec![json!({
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
        })];
        for liquidity_order in liquidity_orders(sell_token, buy_token) {
            orders.push({
                json!({
                    "uid": boundary_liquidity_order(&liquidity_order).uid(),
                    "sellToken": hex_address(liquidity_order.sell.token.into()),
                    "buyToken": hex_address(liquidity_order.buy.token.into()),
                    "sellAmount": liquidity_order.sell.amount.to_string(),
                    "buyAmount": liquidity_order.buy.amount.to_string(),
                    "solverFee": "0",
                    "userFee": "0",
                    "validTo": valid_to,
                    "kind": match liquidity_order.side {
                        competition::order::Side::Sell => "sell",
                        competition::order::Side::Buy => "buy",
                    },
                    "owner": hex_address(admin),
                    "partiallyFillable": liquidity_order.partially_fillable,
                    "executed": liquidity_order.executed.to_string(),
                    "preInteractions": [],
                    "class": "liquidity",
                    "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "reward": 0.1,
                    "signingScheme": "eip712",
                    "signature": format!("0x{}", hex::encode(boundary_liquidity_order(&liquidity_order).signature()))
                })
            });
        }
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
                    "orders": orders,
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
