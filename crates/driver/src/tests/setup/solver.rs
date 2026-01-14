use {
    super::{
        Partial,
        blockchain::{self, Blockchain},
        fee,
    },
    crate::{
        domain::{
            competition::order,
            eth,
            time::{self},
        },
        infra::{self, Ethereum, blockchain::contracts::Addresses, config::file::FeeHandler},
        tests::setup::blockchain::Trade,
    },
    alloy::{primitives::Address, signers::local::PrivateKeySigner},
    const_hex::ToHexExt,
    contracts::alloy::ERC20,
    itertools::Itertools,
    serde_json::{Value, json},
    serde_with::{DisplayFromStr, serde_as},
    solvers_dto::auction::FlashloanHint,
    std::{
        cmp::max,
        collections::{HashMap, HashSet},
        net::SocketAddr,
        sync::{Arc, Mutex},
    },
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
    pub private_key: PrivateKeySigner,
    pub expected_surplus_capturing_jit_order_owners: Vec<Address>,
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
                "sellToken": config.blockchain.get_token(sell_token).encode_hex_with_prefix(),
                "buyToken": config.blockchain.get_token(buy_token).encode_hex_with_prefix(),
                "sellAmount": sell_amount,
                "fullSellAmount": if config.quote { sell_amount } else { quote.sell_amount().to_string() },
                "buyAmount": buy_amount,
                "fullBuyAmount": if config.quote { buy_amount } else { quote.buy_amount().to_string() },
                "validTo": quote.order.valid_to,
                "owner": if config.quote { eth::Address::ZERO } else { quote.order.owner },
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
                    order::Kind::Limit => "limit",
                },
                "appData": app_data::AppDataHash(quote.order.app_data.hash().0.0),
                "signature": if config.quote { "0x".to_string() } else { const_hex::encode_prefixed(quote.order_signature(config.blockchain)) },
                "signingScheme": if config.quote { "eip1271" } else { "eip712" },
            });
            if let Some(receiver) = quote.order.receiver {
                order["receiver"] = json!(receiver.encode_hex_with_prefix());
            }
            if let Some(flashloan) = quote.order.app_data.flashloan() {
                order["flashloanHint"] = json!(FlashloanHint {
                    liquidity_provider: flashloan.liquidity_provider,
                    protocol_adapter: flashloan.protocol_adapter,
                    receiver: flashloan.receiver,
                    token: flashloan.token,
                    amount: flashloan.amount,
                });
            }
            if config.fee_handler == FeeHandler::Solver {
                order.as_object_mut().unwrap().insert(
                    "feePolicies".to_owned(),
                    match quote.order.kind {
                        _ if config.quote => json!([]),
                        order::Kind::Market => json!([]),
                        order::Kind::Limit => {
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
                                    "target": interaction.address.encode_hex_with_prefix(),
                                    "value": "0",
                                    "callData": const_hex::encode_prefixed(&interaction.calldata),
                                    "allowances": [],
                                    "inputs": interaction.inputs.iter().map(|input| {
                                        json!({
                                            "token": eth::Address::from(input.token).encode_hex_with_prefix(),
                                            "amount": input.amount.to_string(),
                                        })
                                    }).collect_vec(),
                                    "outputs": interaction.outputs.iter().map(|output| {
                                        json!({
                                            "token": eth::Address::from(output.token).encode_hex_with_prefix(),
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
                            let trade_json = match fee {
                                Some(fee) => json!({
                                    "kind": "fulfillment",
                                    "order": order,
                                    "executedAmount": executed_amount,
                                    "fee": fee,
                                }),
                                None => json!({
                                    "kind": "fulfillment",
                                    "order": order,
                                    "executedAmount": executed_amount,
                                }),
                            };

                            trades_json.push(trade_json);
                        }
                    }
                    Trade::Jit(jit) => {
                        pre_interactions_json
                            .extend(jit.quoted_order.order.pre_interactions.iter().map(
                            |interaction| {
                                json!({
                                    "kind": "custom",
                                    "internalize": interaction.internalize,
                                    "target": interaction.address.encode_hex_with_prefix(),
                                    "value": "0",
                                    "callData": const_hex::encode_prefixed(&interaction.calldata),
                                    "allowances": [],
                                    "inputs": interaction.inputs.iter().map(|input| {
                                        json!({
                                            "token": eth::Address::from(input.token).encode_hex_with_prefix(),
                                            "amount": input.amount.to_string(),
                                        })
                                    }).collect_vec(),
                                    "outputs": interaction.outputs.iter().map(|output| {
                                        json!({
                                            "token": eth::Address::from(output.token).encode_hex_with_prefix(),
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
                                "target": interaction.address.encode_hex_with_prefix(),
                                "value": "0",
                                "callData": const_hex::encode_prefixed(&interaction.calldata),
                                "allowances": [],
                                "inputs": interaction.inputs.iter().map(|input| {
                                    json!({
                                        "token": eth::Address::from(input.token).encode_hex_with_prefix(),
                                        "amount": input.amount.to_string(),
                                    })
                                }).collect_vec(),
                                "outputs": interaction.outputs.iter().map(|output| {
                                    json!({
                                        "token": eth::Address::from(output.token).encode_hex_with_prefix(),
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
                                "receiver": jit.quoted_order.order.receiver.unwrap_or_default().encode_hex_with_prefix(),
                                "sellAmount": jit.quoted_order.order.sell_amount.to_string(),
                                "buyAmount": jit.quoted_order.order.buy_amount.unwrap_or_default().to_string(),
                                "validTo": jit.quoted_order.order.valid_to,
                                "appData": app_data::AppDataHash(jit.quoted_order.order.app_data.hash().0.0),
                                "kind": match jit.quoted_order.order.side {
                                            order::Side::Sell => "sell",
                                            order::Side::Buy => "buy",
                                },
                                "sellTokenBalance": jit.quoted_order.order.sell_token_source,
                                "buyTokenBalance": jit.quoted_order.order.buy_token_destination,
                                "signature": const_hex::encode_prefixed(jit.quoted_order.order_signature_with_private_key(config.blockchain, config.private_key.clone())),
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
            let mut solution_json = json!({
                "id": i,
                "prices": prices_json,
                "trades": trades_json,
                "interactions": interactions_json,
                "preInteractions": pre_interactions_json,
            });
            if !solution.flashloans.is_empty() {
                solution_json["flashloans"] = serde_json::Value::Object(
                    solution
                        .flashloans
                        .iter()
                        .map(|(order, loan)| {
                            (
                                format!("{:?}", order.0),
                                serde_json::to_value(loan).unwrap(),
                            )
                        })
                        .collect(),
                );
            }
            solutions_json.push(solution_json);
        }

        let build_tokens = config
            .solutions
            .iter()
            .flat_map(|s| s.trades.iter())
            .flat_map(|f| {
                let build_token = |token_name: String| async move {
                    let token = config.blockchain.get_token_wrapped(token_name.as_str());
                    let contract = ERC20::Instance::new(token, config.blockchain.web3.alloy.clone());
                    let settlement = config.blockchain.settlement.address();
                    (
                        token.encode_hex_with_prefix(),
                        json!({
                            "decimals": contract.decimals().call().await.ok(),
                            "symbol": contract.symbol().call().await.ok(),
                            "referencePrice": if config.quote { None } else { Some("1000000000000000000") },
                            // available balance might break if one test settles 2 auctions after
                            // another
                            "availableBalance": contract.balanceOf(*settlement).call().await.unwrap().to_string(),
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
        let rpc = infra::blockchain::Rpc::try_new(infra::blockchain::RpcArgs {
            url,
            max_batch_size: 20,
            max_concurrent_requests: 10,
        })
        .await
        .unwrap();
        let gas = Arc::new(
            infra::blockchain::GasPriceEstimator::new(
                rpc.web3(),
                &Default::default(),
                &[infra::mempool::Config {
                    min_priority_fee: Default::default(),
                    gas_price_cap: eth::U256::from(1000000000000_u128),
                    target_confirm_time: Default::default(),
                    retry_interval: Default::default(),
                    name: "default_rpc".to_string(),
                    max_additional_tip: eth::U256::from(3000000000_u128),
                    additional_tip_percentage: 0.,
                    revert_protection: infra::mempool::RevertProtection::Disabled,
                    nonce_block_number: None,
                    url: config.blockchain.web3_url.parse().unwrap(),
                }],
            )
            .await
            .unwrap(),
        );
        let eth = Ethereum::new(
            rpc,
            Addresses {
                settlement: Some((*config.blockchain.settlement.address()).into()),
                weth: Some((*config.blockchain.weth.address()).into()),
                balances: Some((*config.blockchain.balances.address()).into()),
                signatures: Some((*config.blockchain.signatures.address()).into()),
                cow_amm_helper_by_factory: Default::default(),
                flashloan_router: Some((*config.blockchain.flashloan_router.address()).into()),
            },
            gas,
            eth::U256::from(45_000_000),
            &shared::current_block::Arguments {
                block_stream_poll_interval: None,
                node_ws_url: Some(config.blockchain.web3_ws_url.parse().unwrap()),
            },
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
                    check_solve_request(req, expected);
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

/// Checks the provider /solve request against the expected values while keeping
/// some slack for the effective gas price, as it might vary between blockchain
/// requests.
///
/// Context: when the gas-estimation crate was removed, the Alloy and Web3
/// estimators started failing the driver tests: the request's effective gas
/// value did not match the expected. This did not happen with the previous
/// native estimator because it used a cache, and due to how short the test was
/// the cache always replied with the same value making the test pass. The new
/// estimators do not have a cache, as such the value might change; this check
/// takes that into account and validates the effective gas price within an
/// interval (15% at the time of writing).
fn check_solve_request(request: Value, expected: Value) {
    #[serde_as]
    #[derive(Debug, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SolveRequest {
        #[serde_as(as = "DisplayFromStr")]
        effective_gas_price: u128,
        #[serde(flatten)]
        rest: Value,
    }

    let request: SolveRequest = serde_json::from_value(request).unwrap();
    let expected: SolveRequest = serde_json::from_value(expected).unwrap();
    assert_eq!(
        request.rest, expected.rest,
        "/solve request body does not match expectation"
    );

    const DIFF_PCT: f64 = 0.15; // 15%
    // Assumes the u128 fits inside the i128, in case it doesn't, just upgrade it to
    // U256
    let diff = (request.effective_gas_price as i128 - expected.effective_gas_price as i128).abs();
    let pct = diff as f64 / max(request.effective_gas_price, expected.effective_gas_price) as f64;

    assert!(
        pct < DIFF_PCT,
        "/solve request does not match expectactions, request: {}, expected: {} pct: {pct}, max \
         pct: {DIFF_PCT}",
        request.effective_gas_price,
        expected.effective_gas_price
    );
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
