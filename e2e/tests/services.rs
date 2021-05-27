use contracts::{ERC20Mintable, GPv2Settlement, UniswapV2Factory, UniswapV2Router02, WETH9};
use ethcontract::{
    prelude::{Account, U256},
    H160,
};
use model::DomainSeparator;
use orderbook::{
    account_balances::Web3BalanceFetcher, database::Database, event_updater::EventUpdater,
    fee::EthAwareMinFeeCalculator, metrics::Metrics, orderbook::Orderbook,
};
use prometheus::Registry;
use shared::{
    amm_pair_provider::UniswapPairProvider,
    bad_token::list_based::ListBasedDetector,
    current_block::current_block_stream,
    maintenance::ServiceMaintenance,
    pool_fetching::{CachedPoolFetcher, PoolFetcher},
    price_estimate::BaselinePriceEstimator,
    Web3,
};
use solver::orderbook::OrderBookApi;
use std::{collections::HashSet, str::FromStr, sync::Arc, time::Duration};

pub const API_HOST: &str = "http://127.0.0.1:8080";

#[macro_export]
macro_rules! tx_value {
    ($acc:ident, $value:expr, $call:expr) => {{
        const NAME: &str = stringify!($call);
        $call
            .from($acc.clone())
            .value($value)
            .send()
            .await
            .expect(&format!("{} failed", NAME))
    }};
}
#[macro_export]
macro_rules! tx {
    ($acc:ident, $call:expr) => {
        tx_value!($acc, U256::zero(), $call)
    };
}

pub fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::from(10).pow(18.into())
}

pub fn create_orderbook_api(web3: &Web3, weth_address: H160) -> OrderBookApi {
    let weth = WETH9::at(&web3, weth_address);
    solver::orderbook::OrderBookApi::new(
        reqwest::Url::from_str(API_HOST).unwrap(),
        std::time::Duration::from_secs(10),
        weth,
    )
}

pub struct GPv2 {
    pub settlement: GPv2Settlement,
    pub allowance: H160,
    pub domain_separator: DomainSeparator,
}
impl GPv2 {
    pub async fn fetch(web3: &Web3, designated_solver: &Account) -> Self {
        let settlement = solver::get_settlement_contract(&web3, designated_solver.clone())
            .await
            .expect("Failed to load deployed GPv2Settlement");
        let allowance = settlement
            .allowance_manager()
            .call()
            .await
            .expect("Couldn't get allowance manager address");
        let domain_separator = DomainSeparator(
            settlement
                .domain_separator()
                .call()
                .await
                .expect("Couldn't query domain separator"),
        );
        Self {
            settlement,
            allowance,
            domain_separator,
        }
    }
}

pub struct UniswapContracts {
    pub uniswap_factory: UniswapV2Factory,
    pub uniswap_router: UniswapV2Router02,
}
impl UniswapContracts {
    pub async fn fetch(web3: &Web3) -> Self {
        let uniswap_factory = UniswapV2Factory::deployed(&web3)
            .await
            .expect("Failed to load deployed UniswapFactory");
        let uniswap_router = UniswapV2Router02::deployed(&web3)
            .await
            .expect("Failed to load deployed UniswapRouter");
        Self {
            uniswap_factory,
            uniswap_router,
        }
    }
}

pub async fn deploy_mintable_token(web3: &Web3) -> ERC20Mintable {
    ERC20Mintable::builder(web3)
        .deploy()
        .await
        .expect("MintableERC20 deployment failed")
}

pub struct OrderbookServices {
    pub price_estimator: Arc<BaselinePriceEstimator>,
    pub maintenance: ServiceMaintenance,
}
impl OrderbookServices {
    pub async fn new(
        web3: &Web3,
        gpv2: &GPv2,
        uniswap_factory: &UniswapV2Factory,
        native_token: H160,
    ) -> Self {
        let chain_id = web3
            .eth()
            .chain_id()
            .await
            .expect("Could not get chainId")
            .as_u64();
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let event_updater = Arc::new(EventUpdater::new(gpv2.settlement.clone(), db.clone(), None));
        let current_block_stream = current_block_stream(web3.clone(), Duration::from_secs(1))
            .await
            .unwrap();
        let pair_provider = Arc::new(UniswapPairProvider {
            factory: uniswap_factory.clone(),
            chain_id,
        });
        let pool_fetcher = CachedPoolFetcher::new(
            Box::new(PoolFetcher {
                pair_provider,
                web3: web3.clone(),
            }),
            current_block_stream,
        );
        let gas_estimator = Arc::new(web3.clone());
        let bad_token_detector = Arc::new(ListBasedDetector::deny_list(Vec::new()));
        let price_estimator = Arc::new(BaselinePriceEstimator::new(
            Box::new(pool_fetcher),
            gas_estimator.clone(),
            HashSet::new(),
            bad_token_detector.clone(),
            native_token,
        ));
        let fee_calculator = Arc::new(EthAwareMinFeeCalculator::new(
            price_estimator.clone(),
            gas_estimator,
            native_token,
            db.clone(),
            1.0,
            bad_token_detector.clone(),
        ));
        let orderbook = Arc::new(Orderbook::new(
            gpv2.domain_separator,
            db.clone(),
            Box::new(Web3BalanceFetcher::new(
                web3.clone(),
                gpv2.allowance,
                gpv2.settlement.address(),
            )),
            fee_calculator.clone(),
            Duration::from_secs(120),
            bad_token_detector,
        ));
        let maintenance = ServiceMaintenance {
            maintainers: vec![orderbook.clone(), Arc::new(db.clone()), event_updater],
        };
        let registry = Registry::default();
        let metrics = Arc::new(Metrics::new(&registry).unwrap());
        orderbook::serve_task(
            db.clone(),
            orderbook,
            fee_calculator,
            price_estimator.clone(),
            API_HOST[7..].parse().expect("Couldn't parse API address"),
            registry,
            metrics,
        );

        Self {
            price_estimator,
            maintenance,
        }
    }
}
