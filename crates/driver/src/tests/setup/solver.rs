use {
    super::{
        blockchain::{self, Blockchain},
        fee,
        Partial,
    },
    crate::{
        domain::{
            competition::order,
            eth,
            time::{self},
        },
        infra::{self, blockchain::contracts::Addresses, config::file::FeeHandler, Ethereum},
        tests::{hex_address, setup::blockchain::Trade},
    },
    ethereum_types::H160,
    itertools::Itertools,
    serde_json::json,
    std::{
        collections::{HashMap, HashSet},
        net::SocketAddr,
        sync::{Arc, Mutex},
    },
    web3::signing::Key,
};

pub const NAME: &str = "test-solver";

pub struct Solver {
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub struct Config<'a> {
    pub blockchain: &'a Blockchain,
    pub solutions: &'a [blockchain::Solution],
    pub trusted: &'a HashSet<&'static str>,
    pub quoted_orders: &'a [super::blockchain::QuotedOrder],
    pub deadline: time::Deadline,
    /// Is this a test for the /quote endpoint?
    pub quote: bool,
    pub fee_handler: FeeHandler,
    pub private_key: ethcontract::PrivateKey,
    pub expected_surplus_capturing_jit_order_owners: Vec<H160>,
    pub allow_multiple_solve_requests: bool,
}

impl Solver {
    /// Set up an HTTP server exposing a solver API and acting as a solver mock.
    pub async fn new(config: Config<'_>) -> Self {
        let mut solutions_json = Vec::new();
        let mut orders_json = Vec::new();
        for quote in config.quoted_orders.iter().filter(|q| !q.order.filtered) {
            // ETH orders get unwrapped into WETH by the driver before being passed to the
            // solver.
            let sell_token = if quote.order.sell_token == "ETH" {
                "WETH"
            } else {
                quote.order.sell_token
            };
            let buy_token = if quote.order.buy_token == "ETH" {
                "WETH"
            } else {
                quote.order.buy_token
            };
            let sell_amount = match quote.order.side {
                order::Side::Buy if config.quote => {
                    "22300745198530623141535718272648361505980416".to_owned()
                }
                order::Side::Buy => {
                    let mut current_sell_amount = quote.sell_amount();
                    for fee_policy in &quote.order.fee_policy {
                        match fee_policy {
                            // If the fees are handled in the driver, for volume based fee, we
                            // artificially reduce the limit sell amount
                            // for buy orders before sending to solvers. This
                            // allows driver to withhold volume based fee and not violate original
                            // limit prices.
                            fee::Policy::Volume { factor }
                                if config.fee_handler == FeeHandler::Driver =>
                            {
                                current_sell_amount = eth::TokenAmount(current_sell_amount)
                                    .apply_factor(1.0 / (1.0 + factor))
                                    .unwrap()
                                    .0;
                            }
                            _ => {}
                        }
                    }
                    current_sell_amount.to_string()
                }
                _ => quote.sell_amount().to_string(),
            };
            let buy_amount = match quote.order.side {
                order::Side::Sell if config.quote => "1".to_owned(),
                order::Side::Sell => {
                    let mut current_buy_amount = quote.buy_amount();
                    for fee_policy in &quote.order.fee_policy {
                        match fee_policy {
                            // If the fees are handled in the driver, for volume based fee, we
                            // artificially increase the limit buy
                            // amount for sell orders before sending to solvers. This
                            // allows driver to withhold volume based fee and not violate original
                            // limit prices.
                            fee::Policy::Volume { factor }
                                if config.fee_handler == FeeHandler::Driver =>
                            {
                                current_buy_amount = eth::TokenAmount(current_buy_amount)
                                    .apply_factor(1.0 / (1.0 - factor))
                                    .unwrap()
                                    .0;
                            }
                            _ => {}
                        }
                    }
                    current_buy_amount.to_string()
                }
                _ => quote.buy_amount().to_string(),
            };

            let mut order = json!({
                "uid": if config.quote { Default::default() } else { quote.order_uid(config.blockchain) },
                "sellToken": hex_address(config.blockchain.get_token(sell_token)),
                "buyToken": hex_address(config.blockchain.get_token(buy_token)),
                "sellAmount": sell_amount,
                "fullSellAmount": if config.quote { sell_amount } else { quote.sell_amount().to_string() },
                "buyAmount": buy_amount,
                "fullBuyAmount": if config.quote { buy_amount } else { quote.buy_amount().to_string() },
                "validTo": quote.order.valid_to,
                "owner": if config.quote { H160::zero() } else { quote.order.owner },
                "preInteractions":  json!([]),
                "postInteractions":  json!([]),
                "sellTokenSource": quote.order.sell_token_source,
                "buyTokenDestination": quote.order.buy_token_destination,
                "kind": match quote.order.side {
                    order::Side::Sell => "sell",
                    order::Side::Buy => "buy",
                },
                "partiallyFillable": matches!(quote.order.partial, Partial::Yes { .. }),
                "class": match quote.order.kind {
                    _ if config.quote => "market",
                    order::Kind::Market => "market",
                    order::Kind::Limit { .. } => "limit",
                },
                "appData": quote.order.app_data,
                "signature": if config.quote { "0x".to_string() } else { format!("0x{}", hex::encode(quote.order_signature(config.blockchain))) },
                "signingScheme": if config.quote { "eip1271" } else { "eip712" },
            });
            if config.fee_handler == FeeHandler::Solver {
                order.as_object_mut().unwrap().insert(
                    "feePolicies".to_owned(),
                    match quote.order.kind {
                        _ if config.quote => json!([]),
                        order::Kind::Market => json!([]),
                        order::Kind::Limit { .. } => {
                            let fee_policies_json: Vec<serde_json::Value> = quote
                                .order
                                .fee_policy
                                .iter()
                                .map(|policy| policy.to_json_value())
                                .collect();
                            json!(fee_policies_json)
                        }
                    },
                );
            }
            orders_json.push(order);
        }
        for (i, solution) in config.solutions.iter().enumerate() {
            let mut pre_interactions_json = Vec::new();
            let mut interactions_json = Vec::new();
            let mut prices_json = HashMap::new();
            let mut trades_json = Vec::new();
            for trade in solution.trades.iter() {
                match trade {
                    Trade::Fulfillment(fulfillment) => {
                        interactions_json.extend(fulfillment.interactions.iter().map(
                            |interaction| {
                                json!({
                                    "kind": "custom",
                                    "internalize": interaction.internalize,
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
                            },
                        ));
                        let previous_value = prices_json.insert(
                            config
                                .blockchain
                                .get_token_wrapped(fulfillment.quoted_order.order.sell_token),
                            fulfillment.execution.buy.to_string(),
                        );
                        assert_eq!(previous_value, None, "existing price overwritten");
                        let previous_value = prices_json.insert(
                            config
                                .blockchain
                                .get_token_wrapped(fulfillment.quoted_order.order.buy_token),
                            (fulfillment.execution.sell
                                - fulfillment.quoted_order.order.surplus_fee())
                            .to_string(),
                        );
                        assert_eq!(previous_value, None, "existing price overwritten");
                        {
                            // trades have optional field `fee`
                            let order = if config.quote {
                                Default::default()
                            } else {
                                fulfillment.quoted_order.order_uid(config.blockchain)
                            };
                            let executed_amount = match fulfillment.quoted_order.order.executed {
                                Some(executed) => executed.to_string(),
                                None => match fulfillment.quoted_order.order.side {
                                    order::Side::Sell => (fulfillment.execution.sell
                                        - fulfillment.quoted_order.order.surplus_fee())
                                    .to_string(),
                                    order::Side::Buy => fulfillment.execution.buy.to_string(),
                                },
                            };
                            let fee = fulfillment
                                .quoted_order
                                .order
                                .solver_fee
                                .map(|fee| fee.to_string());
                            match fee {
                                Some(fee) => trades_json.push(json!({
                                    "kind": "fulfillment",
                                    "order": order,
                                    "executedAmount": executed_amount,
                                    "fee": fee,
                                })),
                                None => trades_json.push(json!({
                                    "kind": "fulfillment",
                                    "order": order,
                                    "executedAmount": executed_amount,
                                })),
                            }
                        }
                    }
                    Trade::Jit(jit) => {
                        pre_interactions_json
                            .extend(jit.quoted_order.order.pre_interactions.iter().map(
                            |interaction| {
                                json!({
                                    "kind": "custom",
                                    "internalize": interaction.internalize,
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
                            },
                        ));
                        interactions_json.extend(jit.interactions.iter().map(|interaction| {
                            json!({
                                "kind": "custom",
                                "internalize": interaction.internalize,
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
                        }));
                        // Skipping the prices for JIT orders (non-surplus-capturing)
                        if config
                            .expected_surplus_capturing_jit_order_owners
                            .contains(&jit.quoted_order.order.owner)
                        {
                            let previous_value = prices_json.insert(
                                config
                                    .blockchain
                                    .get_token_wrapped(jit.quoted_order.order.sell_token),
                                jit.execution.buy.to_string(),
                            );
                            assert_eq!(previous_value, None, "existing price overwritten");
                            let previous_value = prices_json.insert(
                                config
                                    .blockchain
                                    .get_token_wrapped(jit.quoted_order.order.buy_token),
                                (jit.execution.sell - jit.quoted_order.order.surplus_fee())
                                    .to_string(),
                            );
                            assert_eq!(previous_value, None, "existing price overwritten");
                        }
                        {
                            let executed_amount = match jit.quoted_order.order.executed {
                                Some(executed) => executed.to_string(),
                                None => match jit.quoted_order.order.side {
                                    order::Side::Sell => (jit.execution.sell
                                        - jit.quoted_order.order.surplus_fee())
                                    .to_string(),
                                    order::Side::Buy => jit.execution.buy.to_string(),
                                },
                            };
                            let mut jit = jit.clone();
                            jit.quoted_order.order = jit
                                .quoted_order
                                .order
                                .receiver(Some(config.private_key.address()));
                            let fee_amount = jit.quoted_order.order.solver_fee.unwrap_or_default();
                            let order = json!({
                                "sellToken": config.blockchain.get_token(jit.quoted_order.order.sell_token),
                                "buyToken": config.blockchain.get_token(jit.quoted_order.order.buy_token),
                                "receiver": hex_address(jit.quoted_order.order.receiver.unwrap_or_default()),
                                "sellAmount": jit.quoted_order.order.sell_amount.to_string(),
                                "buyAmount": jit.quoted_order.order.buy_amount.unwrap_or_default().to_string(),
                                "validTo": jit.quoted_order.order.valid_to,
                                "appData": jit.quoted_order.order.app_data,
                                "kind": match jit.quoted_order.order.side {
                                            order::Side::Sell => "sell",
                                            order::Side::Buy => "buy",
                                },
                                "sellTokenBalance": jit.quoted_order.order.sell_token_source,
                                "buyTokenBalance": jit.quoted_order.order.buy_token_destination,
                                "signature": format!("0x{}", hex::encode(jit.quoted_order.order_signature_with_private_key(config.blockchain, &config.private_key))),
                                "signingScheme": if config.quote { "eip1271" } else { "eip712" },
                            });
                            trades_json.push(json!({
                                "kind": "jit",
                                "order": order,
                                "executedAmount": executed_amount,
                                "fee": fee_amount.to_string(),
                            }));
                        }
                    }
                }
            }
            solutions_json.push(json!({
                "id": i,
                "prices": prices_json,
                "trades": trades_json,
                "interactions": interactions_json,
                "preInteractions": pre_interactions_json,
            }));
        }

        let build_tokens = config
            .solutions
            .iter()
            .flat_map(|s| s.trades.iter())
            .flat_map(|f| {
                let build_token = |token_name: String| async move {
                    let token = config.blockchain.get_token_wrapped(token_name.as_str());
                    let contract = contracts::ERC20::at(&config.blockchain.web3, token);
                    let settlement = config.blockchain.settlement.address();
                    (
                        hex_address(token),
                        json!({
                            "decimals": contract.decimals().call().await.ok(),
                            "symbol": contract.symbol().call().await.ok(),
                            "referencePrice": if config.quote { None } else { Some("1000000000000000000") },
                            // available balance might break if one test settles 2 auctions after
                            // another
                            "availableBalance": contract.balance_of(settlement).call().await.unwrap().to_string(),
                            "trusted": config.trusted.contains(token_name.as_str()),
                        }),
                    )
                };
                let order = match f {
                    Trade::Fulfillment(fulfillment) => &fulfillment.quoted_order.order,
                    Trade::Jit(jit) => &jit.quoted_order.order
                };
                [
                    build_token(order.sell_token.to_string()),
                    build_token(order.buy_token.to_string()),
                ]
            });
        let tokens_json = futures::future::join_all(build_tokens)
            .await
            .into_iter()
            .collect::<HashMap<_, _>>();

        let url = config.blockchain.web3_url.parse().unwrap();
        let rpc = infra::blockchain::Rpc::try_new(&url).await.unwrap();
        let gas = Arc::new(
            infra::blockchain::GasPriceEstimator::new(
                rpc.web3(),
                &Default::default(),
                &[infra::mempool::Config {
                    min_priority_fee: Default::default(),
                    gas_price_cap: eth::U256::MAX,
                    target_confirm_time: Default::default(),
                    retry_interval: Default::default(),
                    kind: infra::mempool::Kind::Public {
                        max_additional_tip: 0.into(),
                        additional_tip_percentage: 0.,
                        revert_protection: infra::mempool::RevertProtection::Disabled,
                    },
                }],
            )
            .await
            .unwrap(),
        );
        let eth = Ethereum::new(
            rpc,
            Addresses {
                settlement: Some(config.blockchain.settlement.address().into()),
                weth: Some(config.blockchain.weth.address().into()),
                cow_amms: vec![],
            },
            gas,
            None,
        )
        .await;

        let state = Arc::new(Mutex::new(StateInner {
            called: false,
            allow_multiple_solve_requests: config.allow_multiple_solve_requests,
        }));
        let app = axum::Router::new()
        .route(
            "/solve",
            axum::routing::post(
                move |axum::extract::State(state): axum::extract::State<State>,
                 axum::extract::Json(req): axum::extract::Json<serde_json::Value>| async move {
                    let effective_gas_price = eth
                        .gas_price()
                        .await
                        .unwrap()
                        .effective()
                        .0
                        .0
                        .to_string();
                    let expected = json!({
                        "id": (!config.quote).then_some("1"),
                        "tokens": tokens_json,
                        "orders": orders_json,
                        "liquidity": [],
                        "effectiveGasPrice": effective_gas_price,
                        "deadline": config.deadline.solvers(),
                        "surplusCapturingJitOrderOwners": config.expected_surplus_capturing_jit_order_owners,
                    });
                    assert_eq!(req, expected, "unexpected /solve request");
                    let mut state = state.0.lock().unwrap();
                    assert!(
                        !state.called || state.allow_multiple_solve_requests,
                        "can't call /solve multiple times"
                    );
                    state.called = true;
                    axum::response::Json(json!({
                        "solutions": solutions_json,
                    }))
                },
            ),
        )
        .with_state(State(state));
        let server =
            axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
        let addr = server.local_addr();
        tokio::spawn(async move { server.await.unwrap() });
        Self { addr }
    }
}

#[derive(Debug, Clone)]
struct StateInner {
    /// Has this solver been called yet? If so, attempting to make another call
    /// will result in a failed test.
    called: bool,
    /// In case you want to allow calling a solver multiple times.
    allow_multiple_solve_requests: bool,
}

#[derive(Debug, Clone)]
struct State(Arc<Mutex<StateInner>>);
