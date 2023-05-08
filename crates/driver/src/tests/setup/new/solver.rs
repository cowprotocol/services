use {
    super::blockchain::{Blockchain, Fulfillment},
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

impl Solver {
    /// Set up an HTTP server exposing a solver API and acting as a solver mock.
    pub async fn new(
        blockchain: &Blockchain,
        solutions: &[Vec<Fulfillment>],
        trusted: &HashSet<&'static str>,
        deadline: chrono::DateTime<chrono::Utc>,
        now: infra::time::Now,
    ) -> Self {
        let mut solutions_json = Vec::new();
        let mut orders_json = Vec::new();
        for (i, fulfillments) in solutions.iter().enumerate() {
            let mut interactions_json = Vec::new();
            let mut prices_json = HashMap::new();
            let mut trades_json = Vec::new();
            for fulfillment in fulfillments {
                let order = &fulfillment.order;
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
                let mut order_json = json!({
                    "uid": fulfillment.order_uid(blockchain, now),
                    "sellToken": hex_address(blockchain.get_token(order.sell_token)),
                    "buyToken": hex_address(blockchain.get_token(order.buy_token)),
                    "sellAmount": fulfillment.sell_amount.to_string(),
                    "buyAmount": fulfillment.buy_amount.to_string(),
                    "feeAmount": fulfillment.order.user_fee.to_string(),
                    "kind": match fulfillment.order.side {
                        order::Side::Sell => "sell",
                        order::Side::Buy => "buy",
                    },
                    "partiallyFillable": matches!(fulfillment.order.partial, order::Partial::Yes { .. }),
                    "class": match fulfillment.order.kind {
                        order::Kind::Market => "market",
                        order::Kind::Liquidity => "liquidity",
                        order::Kind::Limit { .. } => "limit",
                    },
                });
                if let order::Kind::Limit { surplus_fee } = fulfillment.order.kind {
                    order_json
                        .as_object_mut()
                        .unwrap()
                        .insert("surplusFee".to_owned(), surplus_fee.0.to_string().into());
                }
                orders_json.push(order_json);
                prices_json.insert(
                    blockchain.get_token(order.sell_token),
                    fulfillment.buy_amount.to_string(),
                );
                prices_json.insert(
                    blockchain.get_token(order.buy_token),
                    fulfillment.sell_amount.to_string(),
                );
                trades_json.push(json!({
                    "kind": "fulfillment",
                    "order": fulfillment.order_uid(blockchain, now),
                    "executedAmount": match order.side {
                        order::Side::Sell => if order.executed.is_zero() {
                            fulfillment.sell_amount.to_string()
                        } else {
                            order.executed.to_string()
                        },
                        order::Side::Buy => if order.executed.is_zero() {
                            fulfillment.buy_amount.to_string()
                        } else {
                            order.executed.to_string()
                        },
                    },
                }))
            }
            solutions_json.push(json!({
                "id": i,
                "prices": prices_json,
                "trades": trades_json,
                "interactions": interactions_json
            }));
        }
        let tokens_json = solutions
            .iter()
            .flatten()
            .map(|f| &f.order)
            .flat_map(|order| {
                [
                    (
                        hex_address(blockchain.get_token(order.sell_token)),
                        json!({
                            "decimals": null,
                            "symbol": null,
                            "referencePrice": "1000000000000000000",
                            "availableBalance": "0",
                            "trusted": trusted.contains(order.sell_token),
                        }),
                    ),
                    (
                        hex_address(blockchain.get_token(order.buy_token)),
                        json!({
                            "decimals": null,
                            "symbol": null,
                            "referencePrice": "1000000000000000000",
                            "availableBalance": "0",
                            "trusted": trusted.contains(order.buy_token),
                        }),
                    ),
                ]
            })
            .collect::<HashMap<_, _>>();

        let url = blockchain.web3_url.parse().unwrap();
        let eth = Ethereum::ethrpc(
            &url,
            Addresses {
                settlement: Some(blockchain.settlement.address().into()),
                weth: Some(blockchain.weth.address().into()),
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
                        "id": "1",
                        "tokens": tokens_json,
                        "orders": orders_json,
                        "liquidity": [],
                        "effectiveGasPrice": effective_gas_price,
                        "deadline": deadline - auction::Deadline::time_buffer(),
                    });
                    let mut state = state.0.lock().unwrap();
                    assert!(!state.called, "solve was already called");
                    assert_eq!(req, expected, "solve request has unexpected body");
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
