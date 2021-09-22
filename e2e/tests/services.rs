use contracts::{
    BalancerV2Vault, ERC20Mintable, GPv2Settlement, UniswapV2Factory, UniswapV2Router02, WETH9,
};
use ethcontract::{prelude::U256, H160};
use model::DomainSeparator;
use orderbook::{
    account_balances::Web3BalanceFetcher, database::Postgres, event_updater::EventUpdater,
    fee::EthAwareMinFeeCalculator, metrics::Metrics, orderbook::Orderbook,
    solvable_orders::SolvableOrdersCache,
};
use reqwest::Client;
use shared::{
    bad_token::list_based::ListBasedDetector,
    baseline_solver::BaseTokens,
    current_block::{current_block_stream, CurrentBlockStream},
    maintenance::ServiceMaintenance,
    price_estimate::BaselinePriceEstimator,
    recent_block_cache::CacheConfig,
    sources::uniswap::{
        pair_provider::UniswapPairProvider, pool_cache::PoolCache, pool_fetching::PoolFetcher,
    },
    Web3,
};
use solver::orderbook::OrderBookApi;
use std::collections::HashMap;
use std::{num::NonZeroU64, str::FromStr, sync::Arc, time::Duration};

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
    let weth = WETH9::at(web3, weth_address);
    solver::orderbook::OrderBookApi::new(
        reqwest::Url::from_str(API_HOST).unwrap(),
        weth,
        Client::new(),
        Default::default(),
    )
}

pub struct GPv2 {
    pub vault: BalancerV2Vault,
    pub settlement: GPv2Settlement,
    pub native_token: WETH9,
    pub allowance: H160,
    pub domain_separator: DomainSeparator,
}

impl GPv2 {
    pub async fn fetch(web3: &Web3) -> Self {
        let vault = BalancerV2Vault::deployed(web3)
            .await
            .expect("Failed to load deployed BalancerV2Vault");
        let settlement = solver::get_settlement_contract(web3)
            .await
            .expect("Failed to load deployed GPv2Settlement");
        let allowance = settlement
            .vault_relayer()
            .call()
            .await
            .expect("Couldn't get vault relayer address");
        let domain_separator = DomainSeparator(
            settlement
                .domain_separator()
                .call()
                .await
                .expect("Couldn't query domain separator")
                .0,
        );
        let native_token = WETH9::deployed(web3)
            .await
            .expect("Couldn't get deployed WETH contract");
        Self {
            vault,
            settlement,
            native_token,
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
        let uniswap_factory = UniswapV2Factory::deployed(web3)
            .await
            .expect("Failed to load deployed UniswapFactory");
        let uniswap_router = UniswapV2Router02::deployed(web3)
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
    pub block_stream: CurrentBlockStream,
    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub base_tokens: Arc<BaseTokens>,
}

impl OrderbookServices {
    pub async fn new(web3: &Web3, gpv2: &GPv2, uniswap_factory: &UniswapV2Factory) -> Self {
        let metrics = Arc::new(Metrics::new().unwrap());
        let chain_id = web3
            .eth()
            .chain_id()
            .await
            .expect("Could not get chainId")
            .as_u64();
        let db = Arc::new(Postgres::new("postgresql://").unwrap());
        db.clear().await.unwrap();
        let event_updater = Arc::new(EventUpdater::new(
            gpv2.settlement.clone(),
            db.as_ref().clone(),
            None,
        ));
        let pair_provider = Arc::new(UniswapPairProvider {
            factory: uniswap_factory.clone(),
            chain_id,
        });
        let current_block_stream = current_block_stream(web3.clone(), Duration::from_secs(5))
            .await
            .unwrap();
        let pool_fetcher = PoolCache::new(
            CacheConfig {
                number_of_blocks_to_cache: NonZeroU64::new(10).unwrap(),
                number_of_entries_to_auto_update: 20,
                maximum_recent_block_age: 4,
                ..Default::default()
            },
            Box::new(PoolFetcher {
                pair_provider,
                web3: web3.clone(),
            }),
            current_block_stream.clone(),
            metrics.clone(),
        )
        .unwrap();
        let gas_estimator = Arc::new(web3.clone());
        let bad_token_detector = Arc::new(ListBasedDetector::deny_list(Vec::new()));
        let base_tokens = Arc::new(BaseTokens::new(gpv2.native_token.address(), &[]));
        let price_estimator = Arc::new(BaselinePriceEstimator::new(
            Arc::new(pool_fetcher),
            gas_estimator.clone(),
            base_tokens.clone(),
            bad_token_detector.clone(),
            gpv2.native_token.address(),
            1_000_000_000_000_000_000_u128.into(),
        ));
        let fee_calculator = Arc::new(EthAwareMinFeeCalculator::new(
            price_estimator.clone(),
            gas_estimator,
            gpv2.native_token.address(),
            db.clone(),
            0.0,
            bad_token_detector.clone(),
            HashMap::default(),
            1_000_000_000_000_000_000_u128.into(),
        ));
        let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
            web3.clone(),
            Some(gpv2.vault.clone()),
            gpv2.allowance,
            gpv2.settlement.address(),
        ));
        let solvable_orders_cache = SolvableOrdersCache::new(
            Duration::from_secs(120),
            db.clone(),
            balance_fetcher.clone(),
            bad_token_detector.clone(),
            current_block_stream.clone(),
        );
        let orderbook = Arc::new(Orderbook::new(
            gpv2.domain_separator,
            gpv2.settlement.address(),
            db.clone(),
            balance_fetcher,
            fee_calculator.clone(),
            Duration::from_secs(120),
            bad_token_detector,
            Box::new(web3.clone()),
            gpv2.native_token.clone(),
            vec![],
            true,
            solvable_orders_cache.clone(),
            Duration::from_secs(600),
        ));
        let maintenance = ServiceMaintenance {
            maintainers: vec![db.clone(), event_updater],
        };
        orderbook::serve_task(
            db.clone(),
            orderbook,
            fee_calculator,
            price_estimator.clone(),
            API_HOST[7..].parse().expect("Couldn't parse API address"),
            metrics,
        );

        Self {
            price_estimator,
            maintenance,
            block_stream: current_block_stream,
            solvable_orders_cache,
            base_tokens,
        }
    }
}
