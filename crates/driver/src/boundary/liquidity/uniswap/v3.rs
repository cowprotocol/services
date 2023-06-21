use {
    crate::{
        boundary,
        domain::{
            eth,
            liquidity::{
                self,
                uniswap::v3::{Fee, Liquidity, LiquidityNet, Pool, SqrtPrice, Tick},
            },
        },
        infra::{self, blockchain::Ethereum},
    },
    bigdecimal::ToPrimitive,
    contracts::{GPv2Settlement, UniswapV3SwapRouter},
    itertools::Itertools,
    shared::{
        current_block::BlockRetrieving,
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
        liquidity_collector::LiquidityCollecting,
    },
    std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    },
};

pub fn to_domain(id: liquidity::Id, pool: ConcentratedLiquidity) -> Option<liquidity::Liquidity> {
    let handler = pool
        .settlement_handling
        .as_any()
        .downcast_ref::<uniswap_v3::UniswapV3SettlementHandler>()
        .expect("downcast uniswap settlement handler");

    let liquidity = liquidity::Liquidity {
        id,
        gas: eth::Gas(pool.pool.gas_stats.mean_gas),
        kind: liquidity::Kind::UniswapV3(Pool {
            router: handler.inner.router.address().into(),
            address: pool.pool.address.into(),
            tokens: liquidity::TokenPair::new(
                pool.pool.tokens.get(0)?.id.into(),
                pool.pool.tokens.get(1)?.id.into(),
            )?,
            sqrt_price: SqrtPrice(pool.pool.state.sqrt_price),
            liquidity: Liquidity(pool.pool.state.liquidity.as_u128()),
            tick: Tick(pool.pool.state.tick.to_i32()?),
            liquidity_net: pool
                .pool
                .state
                .liquidity_net
                .iter()
                .map(|(key, value)| -> Option<_> {
                    Some((Tick(key.to_i32()?), LiquidityNet(value.to_i128()?)))
                })
                .collect::<Option<BTreeMap<_, _>>>()?,
            fee: Fee(pool.pool.state.fee),
        }),
    };

    Some(liquidity)
}

pub fn to_interaction(
    pool: &liquidity::uniswap::v3::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    let web3 = shared::ethrpc::dummy::web3();

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

    interaction
        .encode()
        .into_iter()
        .map(|(target, value, call_data)| eth::Interaction {
            target: eth::Address(target),
            value: eth::Ether(value),
            call_data: call_data.0.into(),
        })
        .exactly_one()
        .unwrap()
}

pub async fn collector(
    eth: &Ethereum,
    block_retriever: Arc<dyn BlockRetrieving>,
    config: &infra::liquidity::config::UniswapV3,
) -> Box<dyn LiquidityCollecting> {
    let web3 = boundary::web3(eth);
    let router = UniswapV3SwapRouter::at(&web3, config.router.0);

    let pool_fetcher = Arc::new(
        UniswapV3PoolFetcher::new(
            eth.chain_id().0.as_u64(),
            web3.clone(),
            // TODO: Should we rather pass a `reqwest::Client` with preconfigured settings into
            // this function than just creat a default one in place everytime?
            // This could have an impact on things like timeout limits.
            Default::default(),
            block_retriever,
            config.max_pools_to_initialize,
        )
        .await
        .unwrap(),
    );

    Box::new(UniswapV3Liquidity::new(
        router,
        eth.contracts().settlement().clone(),
        web3,
        pool_fetcher,
    ))
}
