use {
    crate::{
        boundary::{self, Result},
        domain::{
            eth,
            liquidity::{self, uniswap},
        },
        infra::{self, blockchain::Ethereum},
    },
    async_trait::async_trait,
    contracts::{GPv2Settlement, IUniswapLikeRouter},
    ethrpc::{block_stream::CurrentBlockWatcher, Web3},
    shared::{
        http_solver::model::TokenAmount,
        sources::uniswap_v2::{
            pair_provider::PairProvider,
            pool_cache::PoolCache,
            pool_fetching::{DefaultPoolReader, PoolFetcher, PoolReading},
        },
    },
    solver::{
        interactions::allowances::{AllowanceManaging, Allowances, Approval, ApprovalRequest},
        liquidity::{uniswap_v2, uniswap_v2::UniswapLikeLiquidity, ConstantProductOrder},
        liquidity_collector::LiquidityCollecting,
    },
    std::{
        collections::HashSet,
        sync::{Arc, Mutex},
    },
};

/// Median gas used per UniswapInteraction (v2).
// estimated with https://dune.com/queries/640717
const GAS_PER_SWAP: u64 = 90_171;

pub fn to_domain(id: liquidity::Id, pool: ConstantProductOrder) -> Result<liquidity::Liquidity> {
    assert!(
        *pool.fee.numer() == 3 && *pool.fee.denom() == 1000,
        "uniswap pools have constant fees",
    );

    Ok(liquidity::Liquidity {
        id,
        gas: GAS_PER_SWAP.into(),
        kind: liquidity::Kind::UniswapV2(to_domain_pool(pool)?),
    })
}

pub fn router(pool: &ConstantProductOrder) -> eth::ContractAddress {
    pool.settlement_handling
        .as_any()
        .downcast_ref::<uniswap_v2::Inner>()
        .expect("downcast uniswap settlement handler")
        .router()
        .address()
        .into()
}

pub(in crate::boundary::liquidity) fn to_domain_pool(
    pool: ConstantProductOrder,
) -> Result<uniswap::v2::Pool> {
    // Trading on Uniswap V2 pools where the reserves overflows `uint112`s does
    // not work, so error if the reserves exceed this maximum.
    let limit = 2_u128.pow(112);
    anyhow::ensure!(
        pool.reserves.0 < limit && pool.reserves.1 < limit,
        "pool reserves overflow uint112",
    );

    Ok(liquidity::uniswap::v2::Pool {
        address: pool.address.into(),
        router: router(&pool),
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
    })
}

pub fn to_interaction(
    pool: &liquidity::uniswap::v2::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    let handler = uniswap_v2::Inner::new(
        IUniswapLikeRouter::at(&ethrpc::dummy::web3(), pool.router.into()),
        GPv2Settlement::at(&ethrpc::dummy::web3(), receiver.0),
        Mutex::new(Allowances::empty(receiver.0)),
    );

    let (_, interaction) = handler.settle(
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

pub async fn collector(
    eth: &Ethereum,
    blocks: &CurrentBlockWatcher,
    config: &infra::liquidity::config::UniswapV2,
) -> Result<Box<dyn LiquidityCollecting>> {
    let eth = eth.with_metric_label("uniswapV2".into());
    collector_with_reader(&eth, blocks, config, DefaultPoolReader::new).await
}

pub(in crate::boundary::liquidity) async fn collector_with_reader<R, F>(
    eth: &Ethereum,
    blocks: &CurrentBlockWatcher,
    config: &infra::liquidity::config::UniswapV2,
    reader: F,
) -> Result<Box<dyn LiquidityCollecting>>
where
    R: PoolReading + Send + Sync + 'static,
    F: FnOnce(Web3, PairProvider) -> R,
{
    let router = eth.contract_at::<IUniswapLikeRouter>(config.router);
    let settlement = eth.contracts().settlement().clone();
    let web3 = router.raw_instance().web3().clone();
    let pool_fetcher = {
        let factory = router.factory().call().await?;
        let pair_provider = PairProvider {
            factory,
            init_code_digest: config.pool_code.into(),
        };

        let pool_fetcher = PoolFetcher::new(
            reader(web3.clone(), pair_provider),
            web3.clone(),
            config.missing_pool_cache_time,
        );

        Arc::new(PoolCache::new(
            boundary::liquidity::cache_config(),
            Arc::new(pool_fetcher),
            blocks.clone(),
        )?)
    };

    Ok(Box::new(UniswapLikeLiquidity::with_allowances(
        router,
        settlement,
        Box::new(NoAllowanceManaging),
        pool_fetcher,
    )))
}

/// An allowance manager that always reports no allowances.
struct NoAllowanceManaging;

#[async_trait]
impl AllowanceManaging for NoAllowanceManaging {
    async fn get_allowances(
        &self,
        _: HashSet<eth::H160>,
        spender: eth::H160,
    ) -> Result<Allowances> {
        Ok(Allowances::empty(spender))
    }

    async fn get_approvals(&self, requests: &[ApprovalRequest]) -> Result<Vec<Approval>> {
        Ok(requests
            .iter()
            .map(|request| Approval {
                spender: request.spender,
                token: request.token,
            })
            .collect())
    }
}
