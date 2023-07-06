use {
    super::{blockchain, blockchain::Blockchain},
    crate::{
        domain::competition::{auction, order},
        infra::{self, blockchain::contracts::Addresses, Ethereum},
        tests::hex_address,
    },
    itertools::Itertools,
    serde_json::json,
    std::{
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
    pub quotes: &'a [super::blockchain::Quote],
    pub deadline: chrono::DateTime<chrono::Utc>,
    /// Is this a test for the /quote endpoint?
    pub quote: bool,
}

impl Solver {
    /// Set up an HTTP server exposing a solver API and acting as a solver mock.
    pub async fn new(config: Config<'_>) -> Self {
        let mut solutions_json = Vec::new();
        let mut orders_json = Vec::new();
        for quote in config.quotes {
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
            orders_json.push(json!({
                "uid": if config.quote { Default::default() } else { quote.order_uid(config.blockchain) },
                "sellToken": hex_address(config.blockchain.get_token(sell_token)),
                "buyToken": hex_address(config.blockchain.get_token(buy_token)),
                "sellAmount": match quote.order.side {
                    order::Side::Buy if config.quote => "6277101735386680763835789423207666416102355444464034512896".to_owned(),
                    _ => (quote.sell_amount() - quote.order.surplus_fee()).to_string(),
                },
                "buyAmount": match quote.order.side {
                    order::Side::Sell if config.quote => "1".to_owned(),
                    _ => quote.buy_amount().to_string(),
                },
                "feeAmount": quote.order.user_fee.to_string(),
                "kind": match quote.order.side {
                    order::Side::Sell => "sell",
                    order::Side::Buy => "buy",
                },
                "partiallyFillable": matches!(quote.order.partial, order::Partial::Yes { .. }),
                "class": match quote.order.kind {
                    _ if config.quote => "market",
                    order::Kind::Market => "market",
                    order::Kind::Liquidity => "liquidity",
                    order::Kind::Limit { .. } => "limit",
                },
            }));
        }
        for (i, solution) in config.solutions.iter().enumerate() {
            let mut interactions_json = Vec::new();
            let mut prices_json = HashMap::new();
            let mut trades_json = Vec::new();
            for fulfillment in solution.fulfillments.iter() {
                interactions_json.extend(fulfillment.interactions.iter().map(|interaction| {
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
                prices_json.insert(
                    config
                        .blockchain
                        .get_token_wrapped(fulfillment.quote.order.sell_token),
                    fulfillment.quote.buy.to_string(),
                );
                prices_json.insert(
                    config
                        .blockchain
                        .get_token_wrapped(fulfillment.quote.order.buy_token),
                    (fulfillment.quote.sell - fulfillment.quote.order.surplus_fee()).to_string(),
                );
                trades_json.push(json!({
                    "kind": "fulfillment",
                    "order": if config.quote { Default::default() } else { fulfillment.quote.order_uid(config.blockchain) },
                    "executedAmount":
                        match fulfillment.quote.order.executed {
                            Some(executed) => executed.to_string(),
                            None => match fulfillment.quote.order.side {
                                order::Side::Sell =>
                                    (fulfillment.quote.sell_amount() - fulfillment.quote.order.surplus_fee()).to_string(),
                                order::Side::Buy => fulfillment.quote.buy_amount().to_string(),
                            },
                        }
                }))
            }
            solutions_json.push(json!({
                "id": i,
                "prices": prices_json,
                "trades": trades_json,
                "interactions": interactions_json,
                "risk": solution.risk.to_string(),
            }));
        }
        let tokens_json = if config.quote {
            Default::default()
        } else {
            config
                .solutions
                .iter()
                .flat_map(|s| s.fulfillments.iter())
                .flat_map(|f| {
                    let quote = &f.quote;
                    [
                        (
                            hex_address(
                                config.blockchain.get_token_wrapped(quote.order.sell_token),
                            ),
                            json!({
                                "decimals": null,
                                "symbol": null,
                                "referencePrice": "1000000000000000000",
                                "availableBalance": "0",
                                "trusted": config.trusted.contains(quote.order.sell_token),
                            }),
                        ),
                        (
                            hex_address(config.blockchain.get_token_wrapped(quote.order.buy_token)),
                            json!({
                                "decimals": null,
                                "symbol": null,
                                "referencePrice": "1000000000000000000",
                                "availableBalance": "0",
                                "trusted": config.trusted.contains(quote.order.buy_token),
                            }),
                        ),
                    ]
                })
                .collect::<HashMap<_, _>>()
        };

        let url = config.blockchain.web3_url.parse().unwrap();
        let eth = Ethereum::ethrpc(
            &url,
            Addresses {
                settlement: Some(config.blockchain.settlement.address().into()),
                weth: Some(config.blockchain.weth.address().into()),
            },
        )
        .await
        .unwrap();
        let state = Arc::new(Mutex::new(StateInner { called: false }));
        let app = axum::Router::new()
        .route(
            "/",
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
                        "id": if config.quote { None } else { Some("1") },
                        "tokens": tokens_json,
                        "orders": orders_json,
                        "liquidity": [],
                        "effectiveGasPrice": effective_gas_price,
                        "deadline": config.deadline - auction::Deadline::time_buffer() - infra::Solver::http_time_buffer(),
                    });
                    assert_eq!(req, expected, "unexpected /solve request");
                    let mut state = state.0.lock().unwrap();
                    assert!(!state.called, "solve was already called");
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
}

#[derive(Debug, Clone)]
struct State(Arc<Mutex<StateInner>>);
