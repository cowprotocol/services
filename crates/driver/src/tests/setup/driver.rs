use {
    super::{blockchain::Blockchain, Mempool, Partial, Solver, Test},
    crate::{
        domain::competition::order,
        tests::{hex_address, setup::blockchain::Trade},
    },
    rand::seq::SliceRandom,
    serde_json::json,
    std::{io::Write, net::SocketAddr, path::PathBuf},
    tokio::sync::oneshot,
};

pub struct Config {
    /// If specified, the driver will load this config file. Otherwise, a
    /// temporary file will be created with reasonable values.
    pub config_file: Option<PathBuf>,
    pub enable_simulation: bool,
    pub mempools: Vec<Mempool>,
}

pub struct Driver {
    pub addr: SocketAddr,
    _delete_on_drop: Option<tempfile::TempPath>,
}

impl Driver {
    /// Start the driver HTTP server and return the server address.
    pub async fn new(
        config: &Config,
        solvers: &Vec<(Solver, SocketAddr)>,
        blockchain: &Blockchain,
    ) -> Self {
        let (config_file, config_temp_path) = match config.config_file.as_ref() {
            Some(config_file) => (config_file.to_owned(), None),
            None => {
                let config_temp_path = create_config_file(config, solvers, blockchain).await;
                (config_temp_path.to_path_buf(), Some(config_temp_path))
            }
        };
        let (addr_sender, addr_receiver) = oneshot::channel();
        let args = vec![
            "/test/driver/path".to_owned(),
            "--addr".to_owned(),
            "0.0.0.0:0".to_owned(),
            "--ethrpc".to_owned(),
            blockchain.web3_url.clone(),
            "--config".to_owned(),
            config_file.to_str().unwrap().to_owned(),
        ];
        tokio::spawn(crate::run(args.into_iter(), Some(addr_sender)));
        let addr = addr_receiver.await.unwrap();
        Self {
            addr,
            _delete_on_drop: config_temp_path,
        }
    }
}

/// Create a request for the driver /solve endpoint.
pub fn solve_req(test: &Test) -> serde_json::Value {
    let mut tokens_json = Vec::new();
    let mut orders_json = Vec::new();
    // The orders are shuffled before being sent to the driver, to ensure that the
    // driver sorts them correctly before forwarding them to the solver.
    let mut quotes = test.quoted_orders.clone();
    quotes.shuffle(&mut rand::thread_rng());
    for quote in quotes.iter() {
        orders_json.push(json!({
            "uid": quote.order_uid(&test.blockchain),
            "sellToken": hex_address(test.blockchain.get_token(quote.order.sell_token)),
            "buyToken": hex_address(test.blockchain.get_token(quote.order.buy_token)),
            "sellAmount": quote.sell_amount().to_string(),
            "buyAmount": quote.buy_amount().to_string(),
            "protocolFees": match quote.order.kind {
                order::Kind::Market => json!([]),
                order::Kind::Liquidity => json!([]),
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
            "validTo": quote.order.valid_to,
            "kind": match quote.order.side {
                order::Side::Sell => "sell",
                order::Side::Buy => "buy",
            },
            "owner": hex_address(test.trader_address),
            "partiallyFillable": matches!(quote.order.partial, Partial::Yes { .. }),
            "executed": match quote.order.partial {
                Partial::Yes { executed } => executed.to_string(),
                Partial::No => "0".to_owned(),
            },
            "preInteractions": [],
            "postInteractions": [],
            "class": match quote.order.kind {
                order::Kind::Market => "market",
                order::Kind::Liquidity => "liquidity",
                order::Kind::Limit { .. } => "limit",
            },
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "signingScheme": "eip712",
            "signature": format!("0x{}", hex::encode(quote.order_signature(&test.blockchain))),
        }));
    }
    for trade in test.trades.iter() {
        match trade {
            Trade::Fulfillment(fulfillment) => {
                tokens_json.push(json!({
                    "address": hex_address(test.blockchain.get_token_wrapped(fulfillment.quoted_order.order.sell_token)),
                    "price": "1000000000000000000",
                    "trusted": test.trusted.contains(fulfillment.quoted_order.order.sell_token),
                }));
                tokens_json.push(json!({
                    "address": hex_address(test.blockchain.get_token_wrapped(fulfillment.quoted_order.order.buy_token)),
                    "price": "1000000000000000000",
                    "trusted": test.trusted.contains(fulfillment.quoted_order.order.buy_token),
                }));
            }
            Trade::Jit(jit) => {
                tokens_json.push(json!({
                    "address": hex_address(test.blockchain.get_token_wrapped(jit.quoted_order.order.sell_token)),
                    "price": "1000000000000000000",
                    "trusted": test.trusted.contains(jit.quoted_order.order.sell_token),
                }));
                tokens_json.push(json!({
                    "address": hex_address(test.blockchain.get_token_wrapped(jit.quoted_order.order.buy_token)),
                    "price": "1000000000000000000",
                    "trusted": test.trusted.contains(jit.quoted_order.order.buy_token),
                }));
            }
        }
    }
    json!({
        "id": "1",
        "tokens": tokens_json,
        "orders": orders_json,
        "deadline": test.deadline,
        "surplusCapturingJitOrderOwners": test.surplus_capturing_jit_order_owners,
    })
}

/// Create a request for the driver /reveal endpoint.
pub fn reveal_req() -> serde_json::Value {
    json!({
        "solutionId": "0",
    })
}

/// Create a request for the driver /settle endpoint.
pub fn settle_req() -> serde_json::Value {
    json!({
        "solutionId": "0",
    })
}

/// Create a request for the driver /quote endpoint.
pub fn quote_req(test: &Test) -> serde_json::Value {
    if test.quoted_orders.len() != 1 {
        panic!("when testing /quote, there must be exactly one order");
    }

    let quote = test.quoted_orders.first().unwrap();
    json!({
        "sellToken": hex_address(test.blockchain.get_token(quote.order.sell_token)),
        "buyToken": hex_address(test.blockchain.get_token(quote.order.buy_token)),
        "amount": match quote.order.side {
            order::Side::Buy => quote.buy_amount().to_string(),
            order::Side::Sell => quote.sell_amount().to_string(),
        },
        "kind": match quote.order.side {
            order::Side::Sell => "sell",
            order::Side::Buy => "buy",
        },
        "deadline": test.deadline,
    })
}

/// Create the config file for the driver to use.
async fn create_config_file(
    config: &Config,
    solvers: &Vec<(Solver, SocketAddr)>,
    blockchain: &Blockchain,
) -> tempfile::TempPath {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let simulation = if config.enable_simulation {
        ""
    } else {
        r#"disable-access-list-simulation = true
           disable-gas-simulation = "2381500"
           "#
    };
    write!(file, "{simulation}").unwrap();
    write!(
        file,
        r#"[contracts]
           gp-v2-settlement = "{}"
           weth = "{}"

           [submission]
           gas-price-cap = "1000000000000"
           "#,
        hex_address(blockchain.settlement.address()),
        hex_address(blockchain.weth.address())
    )
    .unwrap();

    for mempool in &config.mempools {
        match mempool {
            Mempool::Public => {
                write!(
                    file,
                    r#"[[submission.mempool]]
                    mempool = "public"
                    "#,
                )
                .unwrap();
            }
            Mempool::Private { url } => {
                write!(
                    file,
                    r#"[[submission.mempool]]
                    mempool = "mev-blocker"
                    additional-tip-percentage = 0.0
                    url = "{}"
                    "#,
                    url.clone().unwrap_or(blockchain.web3_url.clone()),
                )
                .unwrap();
            }
        }
    }

    for (solver, addr) in solvers {
        write!(
            file,
            r#"[[solver]]
               name = "{}"
               endpoint = "http://{}"
               absolute-slippage = "{}"
               relative-slippage = "{}"
               account = "0x{}"
               solving-share-of-deadline = {}
               http-time-buffer = "{}ms"
               fee-handler = {}
               merge-solutions = {}
               "#,
            solver.name,
            addr,
            solver
                .slippage
                .absolute
                .map(|abs| abs.0)
                .unwrap_or_default(),
            solver.slippage.relative,
            hex::encode(solver.private_key.secret_bytes()),
            solver.timeouts.solving_share_of_deadline.get(),
            solver.timeouts.http_delay.num_milliseconds(),
            serde_json::to_string(&solver.fee_handler).unwrap(),
            solver.merge_solutions,
        )
        .unwrap();
    }
    file.into_temp_path()
}
