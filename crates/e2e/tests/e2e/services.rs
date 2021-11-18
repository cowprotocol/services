use crate::deploy::Contracts;
use contracts::{ERC20Mintable, WETH9};
use ethcontract::{prelude::U256, H160};
use orderbook::{
    account_balances::Web3BalanceFetcher,
    api::order_validation::OrderValidator,
    api::post_quote::OrderQuoter,
    database::Postgres,
    event_updater::EventUpdater,
    fee::{EthAwareMinFeeCalculator, FeeSubsidyConfiguration},
    orderbook::Orderbook,
    solvable_orders::SolvableOrdersCache,
};
use reqwest::Client;
use shared::{
    bad_token::list_based::ListBasedDetector,
    baseline_solver::BaseTokens,
    current_block::{current_block_stream, CurrentBlockStream},
    maintenance::ServiceMaintenance,
    price_estimation::baseline::BaselinePriceEstimator,
    recent_block_cache::CacheConfig,
    sources::uniswap_v2::{
        self,
        pair_provider::PairProvider,
        pool_cache::{NoopPoolCacheMetrics, PoolCache},
        pool_fetching::PoolFetcher,
    },
    Web3,
};
use solver::{liquidity::order_converter::OrderConverter, orderbook::OrderBookApi};
use std::{future::pending, num::NonZeroU64, str::FromStr, sync::Arc, time::Duration};

pub const API_HOST: &str = "http://127.0.0.1:8080";

#[macro_export]
macro_rules! tx_value {
    ($acc:ident, $value:expr, $call:expr) => {{
        const NAME: &str = stringify!($call);
        $call
            .from($acc.clone())
            .value($value)
            .gas_price(0.0.into())
            .send()
            .await
            .expect(&format!("{} failed", NAME))
    }};
}

#[macro_export]
macro_rules! tx {
    ($acc:ident, $call:expr) => {
        $crate::tx_value!($acc, U256::zero(), $call)
    };
}

pub fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::from(10).pow(18.into())
}

#[allow(dead_code)]
pub fn create_orderbook_api() -> OrderBookApi {
    OrderBookApi::new(reqwest::Url::from_str(API_HOST).unwrap(), Client::new())
}

pub fn create_order_converter(web3: &Web3, weth_address: H160) -> OrderConverter {
    OrderConverter {
        native_token: WETH9::at(web3, weth_address),
        liquidity_order_owners: Default::default(),
        fee_objective_scaling_factor: 1.,
    }
}

pub async fn deploy_mintable_token(web3: &Web3) -> ERC20Mintable {
    ERC20Mintable::builder(web3)
        .deploy()
        .await
        .expect("MintableERC20 deployment failed")
}

pub fn uniswap_pair_provider(contracts: &Contracts) -> PairProvider {
    uniswap_v2::pair_provider_for_factory(contracts.uniswap_factory.address())
}

pub struct OrderbookServices {
    pub price_estimator: Arc<BaselinePriceEstimator>,
    pub maintenance: ServiceMaintenance,
    pub block_stream: CurrentBlockStream,
    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub base_tokens: Arc<BaseTokens>,
}

impl OrderbookServices {
    pub async fn new(web3: &Web3, contracts: &Contracts) -> Self {
        let db = Arc::new(Postgres::new("postgresql://").unwrap());
        db.clear().await.unwrap();
        let event_updater = Arc::new(EventUpdater::new(
            contracts.gp_settlement.clone(),
            db.as_ref().clone(),
            None,
        ));
        let pair_provider = uniswap_pair_provider(contracts);
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
            Arc::new(NoopPoolCacheMetrics),
        )
        .unwrap();
        let gas_estimator = Arc::new(web3.clone());
        let bad_token_detector = Arc::new(ListBasedDetector::deny_list(Vec::new()));
        let base_tokens = Arc::new(BaseTokens::new(contracts.weth.address(), &[]));
        let price_estimator = Arc::new(BaselinePriceEstimator::new(
            Arc::new(pool_fetcher),
            gas_estimator.clone(),
            base_tokens.clone(),
            bad_token_detector.clone(),
            contracts.weth.address(),
            1_000_000_000_000_000_000_u128.into(),
        ));
        let fee_calculator = Arc::new(EthAwareMinFeeCalculator::new(
            price_estimator.clone(),
            gas_estimator,
            contracts.weth.address(),
            db.clone(),
            bad_token_detector.clone(),
            1_000_000_000_000_000_000_u128.into(),
            FeeSubsidyConfiguration {
                fee_factor: 0.,
                ..Default::default()
            },
        ));
        let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
            web3.clone(),
            Some(contracts.balancer_vault.clone()),
            contracts.allowance,
            contracts.gp_settlement.address(),
        ));
        let solvable_orders_cache = SolvableOrdersCache::new(
            Duration::from_secs(120),
            db.clone(),
            balance_fetcher.clone(),
            bad_token_detector.clone(),
            current_block_stream.clone(),
        );
        let order_validator = Arc::new(OrderValidator::new(
            Box::new(web3.clone()),
            contracts.weth.clone(),
            vec![],
            Duration::from_secs(120),
            fee_calculator.clone(),
            bad_token_detector.clone(),
            balance_fetcher,
        ));
        let orderbook = Arc::new(Orderbook::new(
            contracts.domain_separator,
            contracts.gp_settlement.address(),
            db.clone(),
            bad_token_detector,
            true,
            solvable_orders_cache.clone(),
            Duration::from_secs(600),
            order_validator.clone(),
        ));
        let maintenance = ServiceMaintenance {
            maintainers: vec![db.clone(), event_updater],
        };
        let quoter = Arc::new(OrderQuoter::new(
            fee_calculator,
            price_estimator.clone(),
            order_validator,
        ));
        orderbook::serve_api(
            db.clone(),
            orderbook,
            quoter,
            API_HOST[7..].parse().expect("Couldn't parse API address"),
            pending(),
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
