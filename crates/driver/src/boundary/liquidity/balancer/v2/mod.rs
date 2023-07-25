use {
    crate::{
        boundary,
        infra::{self, blockchain::Ethereum},
    },
    contracts::{
        BalancerV2LiquidityBootstrappingPoolFactory,
        BalancerV2StablePoolFactory,
        BalancerV2Vault,
        BalancerV2WeightedPoolFactory,
    },
    shared::{
        current_block::{BlockRetrieving, CurrentBlockStream},
        sources::balancer_v2::{
            pool_fetching::BalancerContracts,
            BalancerFactoryKind,
            BalancerPoolFetcher,
        },
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    },
    solver::{
        liquidity::balancer_v2::BalancerV2Liquidity,
        liquidity_collector::LiquidityCollecting,
    },
    std::sync::Arc,
};

pub mod stable;
pub mod weighted;

pub async fn collector(
    eth: &Ethereum,
    block_stream: &CurrentBlockStream,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::BalancerV2,
) -> Box<dyn LiquidityCollecting> {
    let web3 = boundary::web3(eth);
    let contracts = BalancerContracts {
        vault: BalancerV2Vault::at(&web3, config.vault.into()),
        factories: [
            config
                .weighted
                .iter()
                .map(|&factory| {
                    (
                        BalancerFactoryKind::Weighted,
                        BalancerV2WeightedPoolFactory::at(&web3, factory.into())
                            .raw_instance()
                            .clone(),
                    )
                })
                .collect::<Vec<_>>(),
            config
                .stable
                .iter()
                .map(|&factory| {
                    (
                        BalancerFactoryKind::Stable,
                        BalancerV2StablePoolFactory::at(&web3, factory.into())
                            .raw_instance()
                            .clone(),
                    )
                })
                .collect::<Vec<_>>(),
            config
                .weighted
                .iter()
                .map(|&factory| {
                    (
                        BalancerFactoryKind::LiquidityBootstrapping,
                        BalancerV2LiquidityBootstrappingPoolFactory::at(&web3, factory.into())
                            .raw_instance()
                            .clone(),
                    )
                })
                .collect::<Vec<_>>(),
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));

    let balancer_pool_fetcher = Arc::new(
        BalancerPoolFetcher::new(
            eth.chain_id().into(),
            block_retriever.clone(),
            token_info_fetcher.clone(),
            boundary::liquidity::cache_config(),
            block_stream.clone(),
            boundary::liquidity::http_client(),
            web3.clone(),
            &contracts,
            config.pool_deny_list.clone(),
        )
        .await
        .expect("failed to create Balancer pool fetcher"),
    );

    Box::new(BalancerV2Liquidity::new(
        web3,
        balancer_pool_fetcher,
        eth.contracts().settlement().clone(),
        contracts.vault,
    ))
}
