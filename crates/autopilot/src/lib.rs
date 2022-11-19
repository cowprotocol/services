pub mod arguments;
pub mod auction_transaction;
pub mod database;
pub mod event_updater;
pub mod limit_order_quoter;
pub mod risk_adjusted_rewards;
pub mod solvable_orders;

use crate::{
    database::{
        onchain_order_events::{ethflow_events::EthFlowOnchainOrderParser, OnchainOrderParser},
        Postgres,
    },
    event_updater::{CoWSwapOnchainOrdersContract, EventUpdater, GPv2SettlementContract},
    limit_order_quoter::LimitOrderQuoter,
    solvable_orders::SolvableOrdersCache,
};
use contracts::{
    BalancerV2Vault, CowProtocolToken, CowProtocolVirtualToken, IUniswapV3Factory, WETH9,
};
use ethcontract::{errors::DeployError, BlockId, BlockNumber};
use model::DomainSeparator;
use shared::{
    account_balances::Web3BalanceFetcher,
    bad_token::{
        cache::CachingDetector,
        instrumented::InstrumentedBadTokenDetectorExt,
        list_based::{ListBasedDetector, UnknownTokenStrategy},
        token_owner_finder,
        trace_call::TraceCallDetector,
    },
    baseline_solver::BaseTokens,
    fee_subsidy::{
        config::FeeSubsidyConfiguration, cow_token::CowSubsidy, FeeSubsidies, FeeSubsidizing,
    },
    gas_price::InstrumentedGasEstimator,
    http_client::HttpClientFactory,
    maintenance::{Maintaining, ServiceMaintenance},
    metrics::LivenessChecking,
    oneinch_api::OneInchClientImpl,
    order_quoting::OrderQuoter,
    price_estimation::factory::{self, PriceEstimatorFactory},
    recent_block_cache::CacheConfig,
    signature_validator::Web3SignatureValidator,
    sources::{
        balancer_v2::{pool_fetching::BalancerContracts, BalancerFactoryKind, BalancerPoolFetcher},
        uniswap_v2::pool_cache::PoolCache,
        uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
        BaselineSource, PoolAggregator,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    zeroex_api::DefaultZeroExApi,
};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tracing::Instrument;

struct Liveness {
    solvable_orders_cache: Arc<SolvableOrdersCache>,
    max_auction_age: Duration,
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        let age = self.solvable_orders_cache.last_update_time().elapsed();
        age <= self.max_auction_age
    }
}

/// Assumes tracing and metrics registry have already been set up.
pub async fn main(args: arguments::Arguments) {
    let db = Postgres::new(args.db_url.as_str()).await.unwrap();
    let db_metrics = tokio::task::spawn(crate::database::database_metrics(db.clone()));

    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::ethrpc::web3(
        &args.shared.ethrpc,
        &http_factory,
        &args.shared.node_url,
        "base",
    );

    let current_block_stream = args
        .shared
        .current_block
        .stream(web3.clone())
        .await
        .unwrap();

    let settlement_contract = contracts::GPv2Settlement::deployed(&web3)
        .await
        .expect("Couldn't load deployed settlement");
    let vault_relayer = settlement_contract
        .vault_relayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");
    let native_token = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let vault = match BalancerV2Vault::deployed(&web3).await {
        Ok(contract) => Some(contract),
        Err(DeployError::NotFound(_)) => {
            tracing::warn!("balancer contracts are not deployed on this network");
            None
        }
        Err(err) => panic!("failed to get balancer vault contract: {}", err),
    };
    let uniswapv3_factory = match IUniswapV3Factory::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };

    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    let network = web3
        .net()
        .version()
        .await
        .expect("Failed to retrieve network version ID");
    let network_name = shared::network::network_name(&network, chain_id);

    let signature_validator = Arc::new(Web3SignatureValidator::new(web3.clone()));

    let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
        web3.clone(),
        vault.clone(),
        vault_relayer,
        settlement_contract.address(),
    ));

    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            &http_factory,
            &web3,
            args.shared.gas_estimators.as_slice(),
            args.shared.blocknative_api_key.clone(),
        )
        .await
        .expect("failed to create gas price estimator"),
    );

    let baseline_sources = args.shared.baseline_sources.clone().unwrap_or_else(|| {
        shared::sources::defaults_for_chain(chain_id)
            .expect("failed to get default baseline sources")
    });
    tracing::info!(?baseline_sources, "using baseline sources");
    let (pair_providers, pool_fetchers): (Vec<_>, Vec<_>) =
        shared::sources::uniswap_like_liquidity_sources(&web3, &baseline_sources)
            .await
            .expect("failed to load baseline source pair providers")
            .values()
            .cloned()
            .unzip();

    let base_tokens = Arc::new(BaseTokens::new(
        native_token.address(),
        &args.shared.base_tokens,
    ));
    let mut allowed_tokens = args.allowed_tokens.clone();
    allowed_tokens.extend(base_tokens.tokens().iter().copied());
    allowed_tokens.push(model::order::BUY_ETH_ADDRESS);
    let unsupported_tokens = args.unsupported_tokens.clone();

    let finder = token_owner_finder::init(
        &args.token_owner_finder,
        web3.clone(),
        chain_id,
        &http_factory,
        &pair_providers,
        vault.as_ref(),
        uniswapv3_factory.as_ref(),
        &base_tokens,
    )
    .await
    .expect("failed to initialize token owner finders");

    let trace_call_detector = args.tracing_node_url.as_ref().map(|tracing_node_url| {
        Box::new(CachingDetector::new(
            Box::new(TraceCallDetector {
                web3: shared::ethrpc::web3(
                    &args.shared.ethrpc,
                    &http_factory,
                    tracing_node_url,
                    "trace",
                ),
                finder,
                settlement_contract: settlement_contract.address(),
            }),
            args.token_quality_cache_expiry,
        ))
    });
    let bad_token_detector = Arc::new(
        ListBasedDetector::new(
            allowed_tokens,
            unsupported_tokens,
            trace_call_detector
                .map(|detector| UnknownTokenStrategy::Forward(detector))
                .unwrap_or(UnknownTokenStrategy::Allow),
        )
        .instrumented(),
    );

    let pool_aggregator = PoolAggregator { pool_fetchers };

    let cache_config = CacheConfig {
        number_of_blocks_to_cache: args.shared.pool_cache_blocks,
        number_of_entries_to_auto_update: args.pool_cache_lru_size,
        maximum_recent_block_age: args.shared.pool_cache_maximum_recent_block_age,
        max_retries: args.shared.pool_cache_maximum_retries,
        delay_between_retries: args.shared.pool_cache_delay_between_retries_seconds,
    };
    let pool_fetcher = Arc::new(
        PoolCache::new(
            cache_config,
            Arc::new(pool_aggregator),
            current_block_stream.clone(),
        )
        .expect("failed to create pool cache"),
    );
    let block_retriever = args.shared.current_block.retriever(web3.clone());
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let balancer_pool_fetcher = if baseline_sources.contains(&BaselineSource::BalancerV2) {
        let factories = args
            .shared
            .balancer_factories
            .clone()
            .unwrap_or_else(|| BalancerFactoryKind::for_chain(chain_id));
        let contracts = BalancerContracts::new(&web3, factories).await.unwrap();
        let balancer_pool_fetcher = Arc::new(
            BalancerPoolFetcher::new(
                chain_id,
                block_retriever.clone(),
                token_info_fetcher.clone(),
                cache_config,
                current_block_stream.clone(),
                http_factory.create(),
                web3.clone(),
                &contracts,
                args.shared.balancer_pool_deny_list.clone(),
            )
            .await
            .expect("failed to create Balancer pool fetcher"),
        );
        Some(balancer_pool_fetcher)
    } else {
        None
    };
    let uniswap_v3_pool_fetcher = if baseline_sources.contains(&BaselineSource::UniswapV3) {
        match UniswapV3PoolFetcher::new(
            chain_id,
            web3.clone(),
            http_factory.create(),
            block_retriever,
            args.shared.max_pools_to_initialize_cache,
        )
        .await
        {
            Ok(uniswap_v3_pool_fetcher) => Some(Arc::new(uniswap_v3_pool_fetcher)),
            Err(err) => {
                tracing::error!(
                    "failed to create UniswapV3 pool fetcher in autopilot: {}",
                    err,
                );
                None
            }
        }
    } else {
        None
    };
    let zeroex_api = Arc::new(
        DefaultZeroExApi::new(
            &http_factory,
            args.shared
                .zeroex_url
                .as_deref()
                .unwrap_or(DefaultZeroExApi::DEFAULT_URL),
            args.shared.zeroex_api_key.clone(),
        )
        .unwrap(),
    );
    let one_inch_api = OneInchClientImpl::new(
        args.shared.one_inch_url.clone(),
        http_factory.create(),
        chain_id,
    )
    .map(Arc::new);

    let cow_token = match CowProtocolToken::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };
    let cow_vtoken = match CowProtocolVirtualToken::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };

    let mut price_estimator_factory = PriceEstimatorFactory::new(
        &args.price_estimation,
        &args.shared,
        factory::Network {
            web3: web3.clone(),
            name: network_name.to_string(),
            chain_id,
            native_token: native_token.address(),
            settlement: settlement_contract.address(),
            authenticator: settlement_contract
                .authenticator()
                .call()
                .await
                .expect("failed to query solver authenticator address"),
            base_tokens: base_tokens.clone(),
        },
        factory::Components {
            http_factory: http_factory.clone(),
            bad_token_detector: bad_token_detector.clone(),
            uniswap_v2_pools: pool_fetcher.clone(),
            balancer_pools: balancer_pool_fetcher.clone().map(|a| a as _),
            uniswap_v3_pools: uniswap_v3_pool_fetcher.clone().map(|a| a as _),
            tokens: token_info_fetcher.clone(),
            gas_price: gas_price_estimator.clone(),
            zeroex: zeroex_api.clone(),
            oneinch: one_inch_api.ok().map(|a| a as _),
        },
    )
    .expect("failed to initialize price estimator factory");

    let price_estimator = price_estimator_factory
        .price_estimator(&args.order_quoting.price_estimators)
        .unwrap();
    let native_price_estimator = price_estimator_factory
        .native_price_estimator(&args.native_price_estimators)
        .unwrap();

    let risk_adjusted_rewards = (|| {
        if chain_id != 1 {
            return None;
        }
        let cip_args = [
            args.cip_14_beta,
            args.cip_14_alpha1,
            args.cip_14_alpha2,
            args.cip_14_profit,
            args.cip_14_gas_cap,
            args.cip_14_reward_cap,
        ];
        match cip_args.iter().map(|arg| arg.is_some() as u32).sum::<u32>() {
            0 => return None,
            6 => (),
            _ => panic!("need none or all cip_14 arguments"),
        };
        Some(risk_adjusted_rewards::Calculator {
            config: risk_adjusted_rewards::Configuration {
                beta: args.cip_14_beta.unwrap(),
                alpha1: args.cip_14_alpha1.unwrap(),
                alpha2: args.cip_14_alpha2.unwrap(),
                profit: args.cip_14_profit.unwrap(),
                gas_cap: args.cip_14_gas_cap.unwrap(),
                reward_cap: args.cip_14_reward_cap.unwrap(),
            },
            database: db.clone(),
            cow_token: cow_token
                .as_ref()
                .expect("no cow token on mainnet")
                .address(),
            gas_price: gas_price_estimator.clone(),
            native_price: native_price_estimator.clone(),
        })
    })();

    let sync_start = if args.skip_event_sync {
        web3.eth()
            .block(BlockId::Number(BlockNumber::Latest))
            .await
            .ok()
            .flatten()
            .map(|block| {
                (
                    block.number.expect("number must exist").as_u64(),
                    block.hash.expect("hash must exist"),
                )
            })
    } else {
        None
    };
    let block_retriever = args.shared.current_block.retriever(web3.clone());
    let event_updater = Arc::new(EventUpdater::new(
        GPv2SettlementContract::new(settlement_contract.clone()),
        db.clone(),
        block_retriever.clone(),
        sync_start,
    ));
    let mut maintainers: Vec<Arc<dyn Maintaining>> =
        vec![pool_fetcher.clone(), event_updater, Arc::new(db.clone())];

    let gas_price_estimator = Arc::new(InstrumentedGasEstimator::new(
        shared::gas_price_estimation::create_priority_estimator(
            &http_factory,
            &web3,
            args.shared.gas_estimators.as_slice(),
            args.shared.blocknative_api_key.clone(),
        )
        .await
        .expect("failed to create gas price estimator"),
    ));
    let cow_tokens = match (cow_token, cow_vtoken) {
        (None, None) => None,
        (Some(token), Some(vtoken)) => Some((token, vtoken)),
        _ => panic!("should either have both cow token contracts or none"),
    };
    let cow_subsidy = cow_tokens.map(|(token, vtoken)| {
        tracing::debug!("using cow token contracts for subsidy");
        CowSubsidy::new(
            token,
            vtoken,
            args.order_quoting.cow_fee_factors.unwrap_or_default(),
        )
    });
    let liquidity_order_owners: HashSet<_> = args
        .order_quoting
        .liquidity_order_owners
        .iter()
        .copied()
        .collect();
    let fee_subsidy_config = Arc::new(FeeSubsidyConfiguration {
        fee_discount: args.order_quoting.fee_discount,
        min_discounted_fee: args.order_quoting.min_discounted_fee,
        fee_factor: args.order_quoting.fee_factor,
        liquidity_order_owners: liquidity_order_owners.clone(),
        partner_additional_fee_factors: args.order_quoting.partner_additional_fee_factors.clone(),
    }) as Arc<dyn FeeSubsidizing>;

    let fee_subsidy = match cow_subsidy {
        Some(cow_subsidy) => Arc::new(FeeSubsidies(vec![
            fee_subsidy_config,
            Arc::new(cow_subsidy),
        ])),
        None => fee_subsidy_config,
    };
    let quoter = Arc::new(OrderQuoter::new(
        price_estimator,
        native_price_estimator.clone(),
        gas_price_estimator,
        fee_subsidy,
        Arc::new(db.clone()),
        chrono::Duration::from_std(args.order_quoting.eip1271_onchain_quote_validity_seconds)
            .unwrap(),
        chrono::Duration::from_std(args.order_quoting.presign_onchain_quote_validity_seconds)
            .unwrap(),
    ));

    if args.enable_ethflow_orders {
        // The events from the ethflow contract are read with the more generic contract
        // interface called CoWSwapOnchainOrders.
        let cowswap_onchain_order_contract_for_eth_flow =
            contracts::CoWSwapOnchainOrders::at(&web3, args.ethflow_contract);
        let custom_ethflow_order_parser = EthFlowOnchainOrderParser {};
        let onchain_order_event_parser = OnchainOrderParser::new(
            db.clone(),
            quoter.clone(),
            Box::new(custom_ethflow_order_parser),
            DomainSeparator::new(chain_id, settlement_contract.address()),
            settlement_contract.address(),
            liquidity_order_owners,
        );
        let broadcaster_event_updater = Arc::new(EventUpdater::new(
            CoWSwapOnchainOrdersContract::new(cowswap_onchain_order_contract_for_eth_flow),
            onchain_order_event_parser,
            block_retriever,
            sync_start,
        ));
        maintainers.push(broadcaster_event_updater);
    }
    if let Some(balancer) = balancer_pool_fetcher {
        maintainers.push(balancer);
    }
    if let Some(uniswap_v3) = uniswap_v3_pool_fetcher {
        maintainers.push(uniswap_v3);
    }

    let service_maintainer = ServiceMaintenance::new(maintainers);
    let maintenance_task = tokio::task::spawn(
        service_maintainer.run_maintenance_on_new_block(current_block_stream.clone()),
    );

    let block = current_block_stream.borrow().number;
    let solvable_orders_cache = SolvableOrdersCache::new(
        args.min_order_validity_period,
        db.clone(),
        args.banned_users.iter().copied().collect(),
        balance_fetcher.clone(),
        bad_token_detector.clone(),
        current_block_stream.clone(),
        native_price_estimator.clone(),
        signature_validator.clone(),
        Duration::from_secs(2),
        risk_adjusted_rewards,
        args.ethflow_contract,
        args.max_surplus_fee_age * 2,
        args.limit_order_price_factor
            .try_into()
            .expect("limit order price factor can't be converted to BigDecimal"),
    );
    solvable_orders_cache
        .update(block)
        .await
        .expect("failed to perform initial solvable orders update");
    let liveness = Liveness {
        max_auction_age: args.max_auction_age,
        solvable_orders_cache,
    };
    let serve_metrics = shared::metrics::serve_metrics(Arc::new(liveness), args.metrics_address);

    let auction_transaction_updater = crate::auction_transaction::AuctionTransactionUpdater {
        web3,
        db: db.clone(),
        current_block: current_block_stream,
    };
    let auction_transaction = tokio::task::spawn(
        auction_transaction_updater
            .run_forever()
            .instrument(tracing::info_span!("AuctionTransactionUpdater")),
    );

    if args.enable_limit_orders {
        let domain_separator = DomainSeparator::new(chain_id, settlement_contract.address());
        LimitOrderQuoter {
            limit_order_age: chrono::Duration::from_std(args.max_surplus_fee_age).unwrap(),
            loop_delay: args.max_surplus_fee_age / 2,
            quoter,
            database: db,
            signature_validator,
            domain_separator,
        }
        .spawn();
    }

    tokio::select! {
        result = serve_metrics => panic!("serve_metrics exited {:?}", result),
        result = db_metrics => panic!("db_metrics exited {:?}", result),
        result = maintenance_task => panic!("maintenance exited {:?}", result),
        result = auction_transaction => panic!("auction_transaction exited {:?}", result),
    };
}
