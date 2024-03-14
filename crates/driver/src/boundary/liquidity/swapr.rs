use {
    crate::{
        boundary::{self, Result},
        domain::liquidity::{self, swapr},
        infra::{self, blockchain::Ethereum},
    },
    ethrpc::current_block::CurrentBlockStream,
    shared::sources::{
        swapr::reader::SwaprPoolReader,
        uniswap_v2::pool_fetching::DefaultPoolReader,
    },
    solver::{liquidity::ConstantProductOrder, liquidity_collector::LiquidityCollecting},
};

/// The base unit for basis points, i.e. how many basis points in 100%.
const BPS_BASE: u32 = 10_000;

/// Median gas used per UniswapInteraction (v2).
// estimated with https://dune.com/queries/640717
const GAS_PER_SWAP: u64 = 90_171;

pub fn to_domain(id: liquidity::Id, pool: ConstantProductOrder) -> Result<liquidity::Liquidity> {
    // invalid Swapr fee ratio; does not have exact BPS representation
    anyhow::ensure!(
        (pool.fee.numer() * BPS_BASE) % pool.fee.denom() == 0,
        "invalid Swapr fee ratio; does not have exact BPS representation",
    );

    let bps = (pool.fee.numer() * BPS_BASE) / pool.fee.denom();
    let fee = swapr::Fee::new(bps)?;
    Ok(liquidity::Liquidity {
        id,
        gas: GAS_PER_SWAP.into(),
        kind: liquidity::Kind::Swapr(swapr::Pool {
            base: boundary::liquidity::uniswap::v2::to_domain_pool(pool)?,
            fee,
        }),
    })
}

pub async fn collector(
    eth: &Ethereum,
    blocks: &CurrentBlockStream,
    config: &infra::liquidity::config::Swapr,
) -> Result<Box<dyn LiquidityCollecting>> {
    let eth = eth.with_metric_label("swapr".into());
    boundary::liquidity::uniswap::v2::collector_with_reader(
        &eth,
        blocks,
        &infra::liquidity::config::UniswapV2 {
            router: config.router,
            pool_code: config.pool_code,
            missing_pool_cache_time: config.missing_pool_cache_time,
        },
        |web3, pair_provider| SwaprPoolReader(DefaultPoolReader::new(web3, pair_provider)),
    )
    .await
}
