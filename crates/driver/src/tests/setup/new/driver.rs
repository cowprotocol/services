use {
    super::{
        blockchain::Blockchain,
        solver::{self, Solver},
        Test,
    },
    crate::{
        domain::{competition::order, eth},
        infra,
        tests::hex_address,
    },
    secp256k1::SecretKey,
    serde_json::json,
    std::{io::Write, net::SocketAddr, path::PathBuf},
    tokio::sync::oneshot,
};

pub struct Config {
    /// If specified, the driver will load this config file. Otherwise, a
    /// temporary file will be created with reasonable values.
    pub config_file: Option<PathBuf>,
    pub relative_slippage: f64,
    pub absolute_slippage: eth::U256,
    pub solver_address: eth::H160,
    pub solver_secret_key: SecretKey,
    pub now: infra::time::Now,
}

pub struct Driver {
    pub addr: SocketAddr,
    _delete_on_drop: Option<tempfile::TempPath>,
}

impl Driver {
    /// Start the driver HTTP server and return the server address.
    pub async fn new(config: &Config, solver: &Solver, blockchain: &Blockchain) -> Self {
        let (config_file, config_temp_path) = match config.config_file.as_ref() {
            Some(config_file) => (config_file.to_owned(), None),
            None => {
                let config_temp_path = create_config_file(config, solver, blockchain).await;
                (config_temp_path.to_path_buf(), Some(config_temp_path))
            }
        };
        let (addr_sender, addr_receiver) = oneshot::channel();
        let args = vec![
            "/test/driver/path".to_owned(),
            "--addr".to_owned(),
            "0.0.0.0:0".to_owned(),
            "--ethrpc".to_owned(),
            blockchain.geth.url(),
            "--config".to_owned(),
            config_file.to_str().unwrap().to_owned(),
        ];
        tokio::spawn(crate::run(args.into_iter(), config.now, Some(addr_sender)));
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
    for fulfillment in test.fulfillments.iter() {
        tokens_json.push(json!({
            "address": hex_address(test.blockchain.get_token(fulfillment.order.sell_token)),
            "price": "1000000000000000000",
            "trusted": test.trusted.contains(fulfillment.order.sell_token),
        }));
        tokens_json.push(json!({
            "address": hex_address(test.blockchain.get_token(fulfillment.order.buy_token)),
            "price": "1000000000000000000",
            "trusted": test.trusted.contains(fulfillment.order.buy_token),
        }));
        orders_json.push(json!({
            "uid": fulfillment.order_uid(&test.blockchain, test.now),
            "sellToken": hex_address(test.blockchain.get_token(fulfillment.order.sell_token)),
            "buyToken": hex_address(test.blockchain.get_token(fulfillment.order.buy_token)),
            "sellAmount": fulfillment.sell_amount.to_string(),
            "buyAmount": fulfillment.buy_amount.to_string(),
            "solverFee": "0",
            "userFee": fulfillment.order.user_fee.to_string(),
            "validTo": u32::try_from(test.now.now().timestamp()).unwrap() + fulfillment.order.valid_for.0,
            "kind": match fulfillment.order.side {
                order::Side::Sell => "sell",
                order::Side::Buy => "buy",
            },
            "owner": hex_address(test.trader_address),
            "partiallyFillable": matches!(fulfillment.order.partial, order::Partial::Yes { .. }),
            "executed": fulfillment.order.executed.to_string(),
            "preInteractions": [],
            "postInteractions": [],
            "class": match fulfillment.order.kind {
                order::Kind::Market => "market",
                order::Kind::Liquidity => "liquidity",
                order::Kind::Limit { .. } => "limit",
            },
            "surplusFee": match fulfillment.order.kind {
                order::Kind::Limit { surplus_fee } => Some(surplus_fee.0.to_string()),
                _ => None,
            },
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "signingScheme": "eip712",
            "signature": format!("0x{}", hex::encode(fulfillment.order_signature(&test.blockchain, test.now)))
        }));
    }
    // TODO Just noticed: the driver auction ID is a string, the solver auction ID
    // is a number. We should reconcile this.
    json!({
        "id": 1,
        "tokens": tokens_json,
        "orders": orders_json,
        "deadline": test.deadline,
    })
}

/// Create the config file for the driver to use.
async fn create_config_file(
    config: &Config,
    solver: &Solver,
    blockchain: &Blockchain,
) -> tempfile::TempPath {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(
        file,
        r#"[contracts]
           gp-v2-settlement = "{}"
           weth = "{}"

           [submission]
           gas-price-cap = 1000000000000

           [[submission.mempool]]
           mempool = "public"

           [[solver]]
           name = "{}"
           endpoint = "http://{}"
           absolute-slippage = "{}"
           relative-slippage = "{}"
           private-key = "0x{}"
           "#,
        hex_address(blockchain.settlement.address()),
        hex_address(blockchain.weth.address()),
        solver::NAME,
        solver.addr,
        config.absolute_slippage,
        config.relative_slippage,
        config.solver_secret_key.display_secret(),
    )
    .unwrap();
    file.into_temp_path()
}
