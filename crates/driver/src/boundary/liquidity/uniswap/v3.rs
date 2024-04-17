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
    contracts::{GPv2Settlement, UniswapV3SwapRouter},
    ethrpc::current_block::BlockRetrieving,
    shared::{
        http_solver::model::TokenAmount,
        interaction::Interaction,
        sources::uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
    },
    solver::{
        interactions::allowances::Allowances,
        liquidity::{
            uniswap_v3::{self, UniswapV3Liquidity, UniswapV3SettlementHandler},
            ConcentratedLiquidity,
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
            router: handler.inner.router.address().into(),
            address: pool.pool.address.into(),
            tokens: liquidity::TokenPair::new(
                pool.pool.tokens[0].id.into(),
                pool.pool.tokens[1].id.into(),
            )?,
            sqrt_price: SqrtPrice(pool.pool.state.sqrt_price),
            liquidity: Liquidity(pool.pool.state.liquidity.as_u128()),
            tick: Tick(pool.pool.state.tick.clone().try_into()?),
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
    let web3 = ethrpc::dummy::web3();

    let handler = UniswapV3SettlementHandler::new(
        UniswapV3SwapRouter::at(&web3, pool.router.0),
        GPv2Settlement::at(&web3, receiver.0),
        Mutex::new(Allowances::empty(receiver.0)),
        pool.fee.0,
    );

    let (_, interaction) = handler.settle(
        TokenAmount::new(input.0.token.into(), input.0.amount),
        TokenAmount::new(output.0.token.into(), output.0.amount),
    );

    let encoded = interaction.encode();
    eth::Interaction {
        target: eth::Address(encoded.0),
        value: eth::Ether(encoded.1),
        call_data: crate::util::Bytes(encoded.2 .0),
    }
}

pub fn collector(
    eth: &Ethereum,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::UniswapV3,
) -> Box<dyn LiquidityCollecting> {
    let eth = Arc::new(eth.with_metric_label("uniswapV3".into()));
    let config = Arc::new(Clone::clone(config));
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
    )) as Box<_>
}

async fn init_liquidity(
    eth: &Ethereum,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::UniswapV3,
) -> anyhow::Result<impl LiquidityCollecting> {
    let web3 = boundary::web3(eth);
    let router = UniswapV3SwapRouter::at(&web3, config.router.0);

    let pool_fetcher = Arc::new(
        UniswapV3PoolFetcher::new(
            &config.graph_url,
            web3.clone(),
            boundary::liquidity::http_client(),
            block_retriever,
            config.max_pools_to_initialize,
        )
        .await
        .context("failed to initialise UniswapV3 liquidity")?,
    );

    Ok(UniswapV3Liquidity::new(
        router,
        eth.contracts().settlement().clone(),
        web3,
        pool_fetcher,
    ))
}
