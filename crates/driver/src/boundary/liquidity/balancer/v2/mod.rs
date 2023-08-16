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
    contracts::{
        BalancerV2LiquidityBootstrappingPoolFactory,
        BalancerV2StablePoolFactory,
        BalancerV2Vault,
        BalancerV2WeightedPoolFactory,
        GPv2Settlement,
    },
    shared::{
        current_block::{BlockRetrieving, CurrentBlockStream},
        http_solver::model::TokenAmount,
        sources::balancer_v2::{
            pool_fetching::BalancerContracts,
            BalancerFactoryKind,
            BalancerPoolFetcher,
        },
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    },
    solver::{
        interactions::allowances::Allowances,
        liquidity::{balancer_v2, balancer_v2::BalancerV2Liquidity},
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
    let web3 = shared::ethrpc::dummy::web3();
    let handler = balancer_v2::SettlementHandler::new(
        pool.id.into(),
        // Note that this code assumes `receiver == sender`. This assumption is
        // also baked into the Balancer V2 logic in the `shared` crate, so to
        // change this assumption, we would need to change it there as well.
        GPv2Settlement::at(&web3, receiver.0),
        BalancerV2Vault::at(&web3, pool.vault.into()),
        Arc::new(Allowances::empty(receiver.0)),
    );

    let interaction = handler.swap(
        TokenAmount::new(input.0.token.into(), input.0.amount),
        TokenAmount::new(output.0.token.into(), output.0.amount),
    );

    let (target, value, call_data) = interaction.encode_swap();

    eth::Interaction {
        target: target.into(),
        value: value.into(),
        call_data: call_data.0.into(),
    }
}

pub fn collector(
    eth: &Ethereum,
    block_stream: CurrentBlockStream,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::BalancerV2,
) -> Box<dyn LiquidityCollecting> {
    let eth = Arc::new(eth.clone());
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
    )) as Box<_>
}

async fn init_liquidity(
    eth: &Ethereum,
    block_stream: &CurrentBlockStream,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::BalancerV2,
) -> Result<impl LiquidityCollecting> {
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
                .liquidity_bootstrapping
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
        .context("failed to create balancer pool fetcher")?,
    );

    Ok(BalancerV2Liquidity::new(
        web3,
        balancer_pool_fetcher,
        eth.contracts().settlement().clone(),
        contracts.vault,
    ))
}
