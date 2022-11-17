use crate::deploy::Contracts;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use autopilot::{
    database::onchain_order_events::{
        ethflow_events::EthFlowOnchainOrderParser, OnchainOrderParser,
    },
    event_updater::{CoWSwapOnchainOrdersContract, GPv2SettlementContract},
    limit_order_quoter::LimitOrderQuoter,
    solvable_orders::SolvableOrdersCache,
};
use contracts::{
    CoWSwapOnchainOrders, ERC20Mintable, GnosisSafe, GnosisSafeCompatibilityFallbackHandler, WETH9,
};
use database::quotes::QuoteId;
use ethcontract::{Account, Bytes, H160, H256, U256};
use model::{quote::QuoteSigningScheme, DomainSeparator};
use orderbook::{database::Postgres, orderbook::Orderbook};
use reqwest::{Client, StatusCode};
use shared::{
    account_balances::Web3BalanceFetcher,
    bad_token::list_based::ListBasedDetector,
    baseline_solver::BaseTokens,
    current_block::{current_block_stream, CurrentBlockStream},
    ethrpc::Web3,
    fee_subsidy::Subsidy,
    maintenance::ServiceMaintenance,
    order_quoting::{
        CalculateQuoteError, FindQuoteError, OrderQuoter, OrderQuoting, Quote, QuoteHandler,
        QuoteParameters, QuoteSearchParameters,
    },
    order_validation::{OrderValidPeriodConfiguration, OrderValidator, SignatureConfiguration},
    price_estimation::{
        baseline::BaselinePriceEstimator, native::NativePriceEstimator,
        sanitized::SanitizedPriceEstimator,
    },
    rate_limiter::RateLimiter,
    recent_block_cache::CacheConfig,
    signature_validator::Web3SignatureValidator,
    sources::uniswap_v2::{
        self, pair_provider::PairProvider, pool_cache::PoolCache, pool_fetching::PoolFetcher,
    },
};
use solver::{liquidity::order_converter::OrderConverter, orderbook::OrderBookApi};
use std::{
    collections::HashSet,
    future::pending,
    num::{NonZeroU64, NonZeroUsize},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use web3::signing::{Key as _, SecretKeyRef};

pub const API_HOST: &str = "http://127.0.0.1:8080";

#[macro_export]
macro_rules! tx_value {
    ($acc:expr, $value:expr, $call:expr) => {{
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
    ($acc:expr, $call:expr) => {
        $crate::tx_value!($acc, U256::zero(), $call)
    };
}

#[macro_export]
macro_rules! tx_safe {
    ($acc:ident, $safe:ident, $call:expr) => {{
        let call = $call;
        $crate::tx!(
            $acc,
            $safe.exec_transaction(
                call.tx.to.unwrap(),
                call.tx.value.unwrap_or_default(),
                ::ethcontract::Bytes(call.tx.data.unwrap_or_default().0),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                $crate::services::gnosis_safe_prevalidated_signature($acc.address()),
            )
        );
    }};
}

pub fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::from(10).pow(18.into())
}

/// Generate a Safe "pre-validated" signature.
///
/// This is a special "marker" signature that can be used if the account that
/// is executing the transaction is an owner. For single owner safes, this is
/// the easiest way to execute a transaction as it does not involve any ECDSA
/// signing.
///
/// See:
/// - Documentation: <https://docs.gnosis-safe.io/contracts/signatures#pre-validated-signatures>
/// - Code: <https://github.com/safe-global/safe-contracts/blob/c36bcab46578a442862d043e12a83fec41143dec/contracts/GnosisSafe.sol#L287-L291>
pub fn gnosis_safe_prevalidated_signature(owner: H160) -> Bytes<Vec<u8>> {
    let mut signature = vec![0; 65];
    signature[12..32].copy_from_slice(owner.as_bytes());
    signature[64] = 1;
    Bytes(signature)
}

/// Generate an owner signature for EIP-1271.
///
/// The Gnosis Safe uses off-chain ECDSA signatures from its owners as the
/// signature bytes when validating EIP-1271 signatures. Specifically, it
/// expects a signed EIP-712 `SafeMessage(bytes message)` (where `message` is
/// the 32-byte hash of the data being verified).
///
/// See:
/// - Code: <https://github.com/safe-global/safe-contracts/blob/c36bcab46578a442862d043e12a83fec41143dec/contracts/handler/CompatibilityFallbackHandler.sol#L66-L70>
pub async fn gnosis_safe_eip1271_signature(
    key: SecretKeyRef<'_>,
    safe: &GnosisSafe,
    message_hash: H256,
) -> Vec<u8> {
    let handler =
        GnosisSafeCompatibilityFallbackHandler::at(&safe.raw_instance().web3(), safe.address());

    let signing_hash = handler
        .get_message_hash(Bytes(message_hash.as_bytes().to_vec()))
        .call()
        .await
        .unwrap();

    let signature = key.sign(&signing_hash.0, None).unwrap();

    let mut bytes = vec![0u8; 65];
    bytes[0..32].copy_from_slice(signature.r.as_bytes());
    bytes[32..64].copy_from_slice(signature.s.as_bytes());
    bytes[64] = signature.v as _;

    bytes
}

#[allow(dead_code)]
pub fn create_orderbook_api() -> OrderBookApi {
    OrderBookApi::new(
        reqwest::Url::from_str(API_HOST).unwrap(),
        Client::new(),
        Some("".to_string()),
    )
}

pub fn create_order_converter(web3: &Web3, weth_address: H160) -> Arc<OrderConverter> {
    Arc::new(OrderConverter {
        native_token: WETH9::at(web3, weth_address),
        fee_objective_scaling_factor: 1.,
        min_order_age: Duration::from_secs(0),
    })
}

pub async fn deploy_mintable_token(web3: &Web3) -> ERC20Mintable {
    ERC20Mintable::builder(web3)
        .deploy()
        .await
        .expect("MintableERC20 deployment failed")
}

pub struct WethPoolConfig {
    pub token_amount: U256,
    pub weth_amount: U256,
}

pub struct MintableToken {
    pub contract: ERC20Mintable,
    minter: Account,
}

impl MintableToken {
    pub async fn mint(&self, to: H160, amount: U256) {
        tx!(self.minter, self.contract.mint(to, amount));
    }
}

pub async fn deploy_token_with_weth_uniswap_pool(
    web3: &Web3,
    deployed_contracts: &Contracts,
    pool_config: WethPoolConfig,
) -> MintableToken {
    let token = deploy_mintable_token(web3).await;
    let minter = Account::Local(
        web3.eth().accounts().await.expect("get accounts failed")[0],
        None,
    );

    let WethPoolConfig {
        weth_amount,
        token_amount,
    } = pool_config;

    tx!(minter, token.mint(minter.address(), token_amount));
    tx_value!(minter, weth_amount, deployed_contracts.weth.deposit());

    tx!(
        minter,
        deployed_contracts
            .uniswap_factory
            .create_pair(token.address(), deployed_contracts.weth.address())
    );
    tx!(
        minter,
        token.approve(deployed_contracts.uniswap_router.address(), token_amount)
    );
    tx!(
        minter,
        deployed_contracts
            .weth
            .approve(deployed_contracts.uniswap_router.address(), weth_amount)
    );
    tx!(
        minter,
        deployed_contracts.uniswap_router.add_liquidity(
            token.address(),
            deployed_contracts.weth.address(),
            token_amount,
            weth_amount,
            0_u64.into(),
            0_u64.into(),
            minter.address(),
            U256::max_value(),
        )
    );

    MintableToken {
        contract: token,
        minter,
    }
}

pub fn uniswap_pair_provider(contracts: &Contracts) -> PairProvider {
    PairProvider {
        factory: contracts.uniswap_factory.address(),
        init_code_digest: uniswap_v2::INIT_CODE_DIGEST,
    }
}

pub struct OrderbookServices {
    pub price_estimator: Arc<SanitizedPriceEstimator>,
    pub maintenance: ServiceMaintenance,
    pub block_stream: CurrentBlockStream,
    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub base_tokens: Arc<BaseTokens>,
}

impl OrderbookServices {
    pub async fn new(web3: &Web3, contracts: &Contracts, enable_limit_orders: bool) -> Self {
        let api_db = Arc::new(Postgres::new("postgresql://").unwrap());
        let autopilot_db = autopilot::database::Postgres::new("postgresql://")
            .await
            .unwrap();
        database::clear_DANGER(&api_db.pool).await.unwrap();
        let gpv2_event_updater = Arc::new(autopilot::event_updater::EventUpdater::new(
            GPv2SettlementContract::new(contracts.gp_settlement.clone()),
            autopilot_db.clone(),
            contracts.gp_settlement.clone().raw_instance().web3(),
            None,
        ));
        let pair_provider = uniswap_pair_provider(contracts);
        let current_block_stream = current_block_stream(web3.clone(), Duration::from_secs(5))
            .await
            .unwrap();
        let pool_fetcher = PoolCache::new(
            CacheConfig {
                number_of_blocks_to_cache: NonZeroU64::new(10).unwrap(),
                number_of_entries_to_auto_update: NonZeroUsize::new(20).unwrap(),
                maximum_recent_block_age: 4,
                ..Default::default()
            },
            Arc::new(PoolFetcher::uniswap(pair_provider, web3.clone())),
            current_block_stream.clone(),
        )
        .unwrap();
        let gas_estimator = Arc::new(web3.clone());
        let bad_token_detector = Arc::new(ListBasedDetector::deny_list(Vec::new()));
        let base_tokens = Arc::new(BaseTokens::new(contracts.weth.address(), &[]));
        let price_estimator = Arc::new(SanitizedPriceEstimator::new(
            Box::new(BaselinePriceEstimator::new(
                Arc::new(pool_fetcher),
                gas_estimator.clone(),
                base_tokens.clone(),
                contracts.weth.address(),
                1_000_000_000_000_000_000_u128.into(),
                Arc::new(RateLimiter::from_strategy(
                    Default::default(),
                    "baseline_estimator".into(),
                )),
            )),
            contracts.weth.address(),
            bad_token_detector.clone(),
        ));
        let native_price_estimator = Arc::new(NativePriceEstimator::new(
            price_estimator.clone(),
            contracts.weth.address(),
            1_000_000_000_000_000_000_u128.into(),
        ));
        let quoter = Arc::new(OrderQuoter::new(
            price_estimator.clone(),
            native_price_estimator.clone(),
            gas_estimator,
            Arc::new(Subsidy {
                factor: 0.,
                ..Default::default()
            }),
            api_db.clone(),
            chrono::Duration::seconds(60i64),
            chrono::Duration::seconds(60i64),
        ));
        let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
            web3.clone(),
            Some(contracts.balancer_vault.clone()),
            contracts.allowance,
            contracts.gp_settlement.address(),
        ));
        let signature_validator = Arc::new(Web3SignatureValidator::new(web3.clone()));
        let solvable_orders_cache = SolvableOrdersCache::new(
            Duration::from_secs(120),
            autopilot_db.clone(),
            Default::default(),
            balance_fetcher.clone(),
            bad_token_detector.clone(),
            current_block_stream.clone(),
            native_price_estimator.clone(),
            signature_validator.clone(),
            Duration::from_secs(1),
            None,
            H160::zero(),
            Duration::from_secs(5),
            Default::default(),
        );
        let order_validator = Arc::new(
            OrderValidator::new(
                Box::new(web3.clone()),
                contracts.weth.clone(),
                HashSet::default(),
                HashSet::default(),
                OrderValidPeriodConfiguration::any(),
                SignatureConfiguration::all(),
                bad_token_detector,
                quoter.clone(),
                balance_fetcher,
                signature_validator,
                api_db.clone(),
                1,
            )
            .with_limit_orders(enable_limit_orders),
        );
        let custom_ethflow_order_parser = EthFlowOnchainOrderParser {};
        let chain_id = web3.eth().chain_id().await.unwrap();
        let onchain_order_event_parser = OnchainOrderParser::new(
            autopilot_db.clone(),
            quoter.clone(),
            Box::new(custom_ethflow_order_parser),
            DomainSeparator::new(chain_id.as_u64(), contracts.gp_settlement.address()),
            contracts.gp_settlement.address(),
            HashSet::new(),
        );
        let cow_onchain_order_contract =
            CoWSwapOnchainOrders::at(web3, contracts.ethflow.address());
        let ethflow_event_updater = Arc::new(autopilot::event_updater::EventUpdater::new(
            CoWSwapOnchainOrdersContract::new(cow_onchain_order_contract),
            onchain_order_event_parser,
            contracts.ethflow.clone().raw_instance().web3(),
            None,
        ));
        let orderbook = Arc::new(Orderbook::new(
            contracts.domain_separator,
            contracts.gp_settlement.address(),
            api_db.as_ref().clone(),
            order_validator.clone(),
        ));
        let maintenance = ServiceMaintenance::new(vec![
            Arc::new(autopilot_db.clone()),
            ethflow_event_updater,
            gpv2_event_updater,
        ]);
        let quotes = Arc::new(QuoteHandler::new(order_validator, quoter.clone()));
        orderbook::serve_api(
            api_db.clone(),
            orderbook,
            quotes,
            API_HOST[7..].parse().expect("Couldn't parse API address"),
            pending(),
            api_db.clone(),
            None,
            native_price_estimator,
        );
        LimitOrderQuoter {
            limit_order_age: chrono::Duration::seconds(15),
            loop_delay: Duration::from_secs(1),
            quoter: Arc::new(FixedFeeQuoter {
                quoter,
                fee: 1_000.into(),
            }),
            database: autopilot_db,
        }
        .spawn();

        Self {
            price_estimator,
            maintenance,
            block_stream: current_block_stream,
            solvable_orders_cache,
            base_tokens,
        }
    }
}

/// Returns error if communicating with the api fails or if a timeout is reached.
pub async fn wait_for_solvable_orders(client: &Client, minimum: usize) -> Result<()> {
    let task = async {
        loop {
            let response = client
                .get(format!("{}/api/v1/auction", API_HOST))
                .send()
                .await?;
            match response.status() {
                StatusCode::OK => {
                    let auction: model::auction::AuctionWithId = response.json().await?;
                    if auction.auction.orders.len() >= minimum {
                        return Ok(());
                    }
                }
                StatusCode::NOT_FOUND => (),
                other => anyhow::bail!("unexpected status code {}", other),
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    match tokio::time::timeout(Duration::from_secs(5), task).await {
        Ok(inner) => inner,
        Err(_) => Err(anyhow!("timeout")),
    }
}

/// Same as [`OrderQuoter`], but forces the fee to be exactly the specified amount.
struct FixedFeeQuoter {
    quoter: Arc<OrderQuoter>,
    fee: U256,
}

#[async_trait]
impl OrderQuoting for FixedFeeQuoter {
    /// Computes a quote for the specified order parameters. Doesn't store the quote.
    async fn calculate_quote(
        &self,
        parameters: QuoteParameters,
    ) -> Result<Quote, CalculateQuoteError> {
        self.quoter
            .calculate_quote(parameters)
            .await
            .map(|q| Quote {
                fee_amount: self.fee,
                ..q
            })
    }

    /// Stores a quote.
    async fn store_quote(&self, quote: Quote) -> Result<Quote> {
        self.quoter.store_quote(quote).await
    }

    /// Finds an existing quote.
    async fn find_quote(
        &self,
        id: Option<QuoteId>,
        parameters: QuoteSearchParameters,
        signing_scheme: &QuoteSigningScheme,
    ) -> Result<Quote, FindQuoteError> {
        self.quoter.find_quote(id, parameters, signing_scheme).await
    }
}
