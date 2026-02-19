use {
    crate::{
        boundary::{self, Result},
        domain::{
            eth,
            liquidity::{
                self,
                uniswap::v3::{Fee, Liquidity, LiquidityNet, Pool, SqrtPrice, Tick},
            },
        },
        infra::{self, blockchain::Ethereum},
    },
    anyhow::Context,
    ethrpc::block_stream::BlockRetrieving,
    shared::{
        http_solver::model::TokenAmount,
        interaction::Interaction,
        maintenance::ServiceMaintenance,
        sources::uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
    },
    solver::{
        interactions::allowances::Allowances,
        liquidity::{
            ConcentratedLiquidity,
            uniswap_v3::{self, UniswapV3Liquidity, UniswapV3SettlementHandler},
        },
        liquidity_collector::{BackgroundInitLiquiditySource, LiquidityCollecting},
    },
    std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    },
};

pub fn to_domain(id: liquidity::Id, pool: ConcentratedLiquidity) -> Result<liquidity::Liquidity> {
    anyhow::ensure!(
        pool.pool.tokens.len() == 2,
        "Uniswap V3 pools should have exactly 2 tokens",
    );

    let handler = pool
        .settlement_handling
        .as_any()
        .downcast_ref::<uniswap_v3::UniswapV3SettlementHandler>()
        .expect("downcast uniswap settlement handler");

    Ok(liquidity::Liquidity {
        id,
        gas: eth::Gas(pool.pool.gas_stats.mean_gas),
        kind: liquidity::Kind::UniswapV3(Pool {
            router: handler.inner.router.into(),
            address: pool.pool.address.into(),
            tokens: liquidity::TokenPair::try_new(
                pool.pool.tokens[0].id.into(),
                pool.pool.tokens[1].id.into(),
            )?,
            sqrt_price: SqrtPrice(pool.pool.state.sqrt_price),
            liquidity: Liquidity(u128::try_from(pool.pool.state.liquidity)?),
            tick: Tick(pool.pool.state.tick.try_into()?),
            liquidity_net: pool
                .pool
                .state
                .liquidity_net
                .iter()
                .map(|(key, value)| -> Result<_> {
                    Ok((Tick(key.try_into()?), LiquidityNet(value.try_into()?)))
                })
                .collect::<Result<BTreeMap<_, _>>>()?,
            fee: Fee(pool.pool.state.fee),
        }),
    })
}

pub fn to_interaction(
    pool: &liquidity::uniswap::v3::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    let handler = UniswapV3SettlementHandler::new(
        pool.router.into(),
        *receiver,
        Mutex::new(Allowances::empty(*receiver)),
        pool.fee.0,
    );

    let (_, interaction) = handler.settle(
        TokenAmount::new(input.0.token.into(), input.0.amount),
        TokenAmount::new(output.0.token.into(), output.0.amount),
    );

    let encoded = interaction.encode();
    eth::Interaction {
        target: encoded.0,
        value: encoded.1.into(),
        call_data: crate::util::Bytes(encoded.2.0.to_vec()),
    }
}

pub fn collector(
    eth: &Ethereum,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::UniswapV3,
) -> Box<dyn LiquidityCollecting> {
    let eth = Arc::new(eth.with_metric_label("uniswapV3".into()));
    let config = Arc::new(Clone::clone(config));
    let reinit_interval = config.reinit_interval;
    let init = move || {
        let eth = eth.clone();
        let block_retriever = block_retriever.clone();
        let config = config.clone();
        async move { init_liquidity(&eth, block_retriever.clone(), &config).await }
    };
    const TEN_MINUTES: std::time::Duration = std::time::Duration::from_secs(10 * 60);
    Box::new(BackgroundInitLiquiditySource::new(
        "uniswap-v3",
        init,
        TEN_MINUTES,
        reinit_interval,
    )) as Box<_>
}

async fn init_liquidity(
    eth: &Ethereum,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::UniswapV3,
) -> anyhow::Result<impl LiquidityCollecting + use<>> {
    let web3 = eth.web3().clone();

    let pool_fetcher = Arc::new(
        UniswapV3PoolFetcher::new(
            &config.graph_url,
            web3.clone(),
            boundary::liquidity::http_client(),
            block_retriever,
            config.max_pools_to_initialize,
            config.max_pools_per_tick_query,
        )
        .await
        .context("failed to initialise UniswapV3 liquidity")?,
    );

    let update_task = ServiceMaintenance::new(vec![pool_fetcher.clone()])
        .run_maintenance_on_new_block(eth.current_block().clone());
    tokio::task::spawn(update_task);

    Ok(UniswapV3Liquidity::new(
        config.router.into(),
        *eth.contracts().settlement().address(),
        web3,
        pool_fetcher,
    ))
}
