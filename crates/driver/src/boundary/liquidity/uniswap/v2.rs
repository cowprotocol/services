use {
    crate::{
        boundary,
        domain::{eth, liquidity},
        infra::blockchain::{contracts::ContractAt, Ethereum},
    },
    anyhow::Result,
    contracts::{GPv2Settlement, IUniswapLikeRouter},
    futures::StreamExt,
    shared::{
        current_block::{self, CurrentBlockStream},
        maintenance::Maintaining,
        price_estimation,
        sources::uniswap_v2::{
            pair_provider::PairProvider,
            pool_cache::PoolCache,
            pool_fetching::{DefaultPoolReader, PoolFetcher, PoolReading},
        },
    },
    solver::{
        interactions::allowances::{Allowances, NoAllowanceManaging},
        liquidity::{uniswap_v2, uniswap_v2::UniswapLikeLiquidity, ConstantProductOrder},
        liquidity_collector::LiquidityCollecting,
    },
    std::{
        sync,
        sync::{Arc, Mutex},
    },
    tracing::Instrument,
    web3::Web3,
};

pub fn to_domain(id: liquidity::Id, pool: ConstantProductOrder) -> liquidity::Liquidity {
    assert!(
        *pool.fee.numer() == 3 && *pool.fee.denom() == 1000,
        "uniswap pools have constant fees",
    );

    let handler = pool
        .settlement_handling
        .as_any()
        .downcast_ref::<uniswap_v2::Inner>()
        .expect("downcast uniswap settlment handler");

    liquidity::Liquidity {
        id,
        address: pool.address.into(),
        gas: price_estimation::gas::GAS_PER_UNISWAP.into(),
        data: liquidity::Data::UniswapV2(liquidity::uniswap::v2::Pool {
            router: handler.router().address().into(),
            reserves: liquidity::uniswap::v2::Reserves::new(
                eth::Asset {
                    token: pool.tokens.get().0.into(),
                    amount: pool.reserves.0.into(),
                },
                eth::Asset {
                    token: pool.tokens.get().1.into(),
                    amount: pool.reserves.1.into(),
                },
            )
            .expect("invalid uniswap token pair"),
        }),
    }
}

pub fn to_interaction(
    pool: &liquidity::uniswap::v2::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    let handler = uniswap_v2::Inner::new(
        IUniswapLikeRouter::at(&shared::ethrpc::dummy::web3(), pool.router.into()),
        GPv2Settlement::at(&shared::ethrpc::dummy::web3(), receiver.0),
        Mutex::new(Allowances::empty(receiver.0)),
    );

    let (_, interaction) = handler.settle(
        (input.0.token.into(), input.0.amount),
        (output.0.token.into(), output.0.amount),
    );

    let (target, value, call_data) = interaction.encode_swap();

    eth::Interaction {
        target: target.into(),
        value: value.into(),
        call_data: call_data.0,
    }
}

pub async fn collector(
    eth: &Ethereum,
    blocks: &CurrentBlockStream,
    config: &liquidity::fetcher::config::UniswapV2,
) -> Result<Box<dyn LiquidityCollecting>> {
    let router = eth.contract_at::<IUniswapLikeRouter>(config.router);
    let settlement = eth.contracts().settlement().clone();
    let web3 = router.raw_instance().web3().clone();
    let pool_fetcher = {
        let factory = router.factory().call().await?;
        let pair_provider = PairProvider {
            factory,
            init_code_digest: config.pool_code.0,
        };
        let pool_reader = DefaultPoolReader::for_pair_provider(pair_provider, web3.clone());

        let pool_fetcher = PoolFetcher {
            pool_reader,
            web3: web3.clone(),
        };

        let pool_cache = Arc::new(PoolCache::new(
            boundary::liquidity::cache_config(),
            Arc::new(pool_fetcher),
            blocks.clone(),
        )?);

        tokio::task::spawn(
            cache_update(blocks.clone(), Arc::downgrade(&pool_cache))
                .instrument(tracing::info_span!("uniswap_v2_cache")),
        );

        pool_cache
    };

    Ok(Box::new(UniswapLikeLiquidity::with_allowances(
        router,
        settlement,
        Box::new(NoAllowanceManaging),
        pool_fetcher,
    )))
}

impl ContractAt for IUniswapLikeRouter {
    fn at(web3: &Web3<web3::transports::Http>, address: eth::ContractAddress) -> Self {
        Self::at(web3, address.0)
    }
}

async fn cache_update(blocks: CurrentBlockStream, pool_cache: sync::Weak<PoolCache>) {
    let mut blocks = current_block::into_stream(blocks);
    loop {
        let block = blocks
            .next()
            .await
            .expect("block stream unexpectedly ended")
            .number;

        let pool_cache = match pool_cache.upgrade() {
            Some(value) => value,
            None => {
                tracing::debug!("pool cache dropped; stopping update task");
                break;
            }
        };

        tracing::info_span!("maintenance", block)
            .in_scope(|| async move {
                if let Err(err) = pool_cache.run_maintenance().await {
                    tracing::warn!(?err, "error updating pool cache");
                }
            })
            .await;
    }
}
