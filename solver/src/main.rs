use ethcontract::PrivateKey;
use reqwest::Url;
use solver::{
    driver::Driver, gas_price_estimation::GasEstimatorType, liquidity::uniswap::UniswapLiquidity,
    naive_solver::NaiveSolver,
};
use std::time::Duration;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Arguments {
    #[structopt(flatten)]
    shared: shared::arguments::Arguments,

    /// The API endpoint to fetch the orderbook
    #[structopt(long, env = "ORDERBOOK_URL", default_value = "http://localhost:8080")]
    orderbook_url: Url,

    /// The timeout for the API endpoint to fetch the orderbook
    #[structopt(
        long,
        env = "ORDERBOOK_TIMEOUT",
        default_value = "10",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    orderbook_timeout: Duration,

    /// The private key used by the driver to sign transactions.
    #[structopt(short = "k", long, env = "PRIVATE_KEY", hide_env_values = true)]
    private_key: PrivateKey,

    /// Which gas estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators support different networks.
    /// `EthGasStation`: supports mainnet.
    /// `GasNow`: supports mainnet.
    /// `GnosisSafe`: supports mainnet and rinkeby.
    /// `Web3`: supports every network.
    #[structopt(
        long,
        env = "GAS_ESTIMATORS",
        default_value = "Web3",
        possible_values = &GasEstimatorType::variants(),
        case_insensitive = true,
        use_delimiter = true
    )]
    gas_estimators: Vec<GasEstimatorType>,

    /// The target confirmation time for settlement transactions used to estimate gas price.
    #[structopt(
        long,
        env = "TARGET_CONFIRM_TIME",
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    target_confirm_time: Duration,
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    shared::tracing::initialize(args.shared.log_filter.as_str());
    tracing::info!("running solver with {:#?}", args);
    // TODO: custom transport that allows setting timeout
    let transport = web3::transports::Http::new(args.shared.node_url.as_str())
        .expect("transport creation failed");
    let web3 = web3::Web3::new(transport);
    let uniswap_router = contracts::UniswapV2Router02::deployed(&web3)
        .await
        .expect("couldn't load deployed uniswap router");
    let uniswap_factory = contracts::UniswapV2Factory::deployed(&web3)
        .await
        .expect("couldn't load deployed uniswap router");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    let settlement_contract = solver::get_settlement_contract(&web3, chain_id, args.private_key)
        .await
        .expect("couldn't load deployed settlement");
    let orderbook_api =
        solver::orderbook::OrderBookApi::new(args.orderbook_url, args.orderbook_timeout);
    let uniswap_liquidity = UniswapLiquidity::new(
        uniswap_factory.clone(),
        uniswap_router.clone(),
        settlement_contract.clone(),
        web3.clone(),
        chain_id,
    );
    let solver = NaiveSolver {
        uniswap_router,
        uniswap_factory,
        gpv2_settlement: settlement_contract.clone(),
    };
    let gas_price_estimator = solver::gas_price_estimation::create_priority_estimator(
        &reqwest::Client::new(),
        &web3,
        args.gas_estimators.as_slice(),
    )
    .await
    .expect("failed to create gas price estimator");
    let mut driver = Driver::new(
        settlement_contract,
        uniswap_liquidity,
        orderbook_api,
        Box::new(solver),
        Box::new(gas_price_estimator),
        args.target_confirm_time,
    );
    driver.run_forever().await;
}
