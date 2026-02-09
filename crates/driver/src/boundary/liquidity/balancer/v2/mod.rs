use {
    crate::{
        boundary,
        domain::{
            eth,
            liquidity::{self, balancer},
        },
        infra::{self, blockchain::Ethereum},
    },
    anyhow::{Context, Result},
    contracts::alloy::{
        BalancerV2ComposableStablePoolFactory,
        BalancerV2LiquidityBootstrappingPoolFactory,
        BalancerV2StablePoolFactoryV2,
        BalancerV2Vault,
        BalancerV2WeightedPoolFactory,
        BalancerV2WeightedPoolFactoryV3,
    },
    ethrpc::block_stream::{BlockRetrieving, CurrentBlockWatcher},
    shared::{
        http_solver::model::TokenAmount,
        sources::balancer_v2::{
            BalancerPoolFetcher,
            pool_fetching::{BalancerContracts, BalancerFactoryInstance},
        },
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    },
    solver::{
        interactions::allowances::Allowances,
        liquidity::balancer_v2::{self, BalancerV2Liquidity},
        liquidity_collector::{BackgroundInitLiquiditySource, LiquidityCollecting},
    },
    std::sync::Arc,
};

pub mod stable;
pub mod weighted;

struct Pool {
    vault: eth::ContractAddress,
    id: balancer::v2::Id,
}

fn to_interaction(
    pool: &Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    let handler = balancer_v2::SettlementHandler::new(
        pool.id.0,
        // Note that this code assumes `receiver == sender`. This assumption is
        // also baked into the Balancer V2 logic in the `shared` crate, so to
        // change this assumption, we would need to change it there as well.
        *receiver,
        pool.vault.0,
        Allowances::empty(*receiver),
    );

    let interaction = handler.swap(
        TokenAmount::new(input.0.token.0.0, input.0.amount.0),
        TokenAmount::new(output.0.token.0.0, output.0.amount.0),
    );

    let (target, value, call_data) = interaction.encode_swap();

    eth::Interaction {
        target,
        value: value.into(),
        call_data: call_data.0.to_vec().into(),
    }
}

pub fn collector(
    eth: &Ethereum,
    block_stream: CurrentBlockWatcher,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::BalancerV2,
) -> Box<dyn LiquidityCollecting> {
    let eth = Arc::new(eth.with_metric_label("balancerV2".into()));
    let reinit_interval = config.reinit_interval;
    let config = Arc::new(config.clone());
    let init = move || {
        let eth = eth.clone();
        let block_stream = block_stream.clone();
        let block_retriever = block_retriever.clone();
        let config = config.clone();
        async move { init_liquidity(&eth, &block_stream, block_retriever.clone(), &config).await }
    };
    const TEN_MINUTES: std::time::Duration = std::time::Duration::from_secs(10 * 60);
    Box::new(BackgroundInitLiquiditySource::new(
        "balancer-v2",
        init,
        TEN_MINUTES,
        reinit_interval,
    )) as Box<_>
}

async fn init_liquidity(
    eth: &Ethereum,
    block_stream: &CurrentBlockWatcher,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::BalancerV2,
) -> Result<impl LiquidityCollecting + use<>> {
    let web3 = eth.web3().clone();
    let contracts = BalancerContracts {
        vault: BalancerV2Vault::Instance::new(config.vault.0, web3.provider.clone()),
        factories: [
            config
                .weighted
                .iter()
                .map(|&factory| {
                    BalancerFactoryInstance::Weighted(BalancerV2WeightedPoolFactory::Instance::new(
                        factory,
                        web3.provider.clone(),
                    ))
                })
                .collect::<Vec<_>>(),
            config
                .weighted_v3plus
                .iter()
                .map(|&factory| {
                    BalancerFactoryInstance::WeightedV3(
                        BalancerV2WeightedPoolFactoryV3::Instance::new(
                            factory,
                            web3.provider.clone(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
            config
                .stable
                .iter()
                .map(|&factory| {
                    BalancerFactoryInstance::StableV2(BalancerV2StablePoolFactoryV2::Instance::new(
                        factory,
                        web3.provider.clone(),
                    ))
                })
                .collect::<Vec<_>>(),
            config
                .liquidity_bootstrapping
                .iter()
                .map(|&factory| {
                    BalancerFactoryInstance::LiquidityBootstrapping(
                        BalancerV2LiquidityBootstrappingPoolFactory::Instance::new(
                            factory,
                            web3.provider.clone(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
            config
                .composable_stable
                .iter()
                .map(|&factory| {
                    BalancerFactoryInstance::ComposableStable(
                        BalancerV2ComposableStablePoolFactory::Instance::new(
                            factory,
                            web3.provider.clone(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Arc::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));

    let balancer_pool_fetcher = Arc::new(
        BalancerPoolFetcher::new(
            &config.graph_url,
            block_retriever.clone(),
            token_info_fetcher.clone(),
            boundary::liquidity::cache_config(),
            block_stream.clone(),
            boundary::liquidity::http_client(),
            web3.clone(),
            &contracts,
            config.pool_deny_list.to_vec(),
        )
        .await
        .context("failed to create balancer pool fetcher")?,
    );

    Ok(BalancerV2Liquidity::new(
        web3,
        balancer_pool_fetcher,
        *eth.contracts().settlement().address(),
        *contracts.vault.address(),
    ))
}
