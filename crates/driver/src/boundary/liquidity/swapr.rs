use {
    crate::{
        boundary,
        domain::liquidity::{self, swapr},
        infra::{self, blockchain::Ethereum},
    },
    anyhow::Result,
    shared::{
        current_block::CurrentBlockStream,
        price_estimation,
        sources::{swapr::reader::SwaprPoolReader, uniswap_v2::pool_fetching::DefaultPoolReader},
    },
    solver::{liquidity::ConstantProductOrder, liquidity_collector::LiquidityCollecting},
};

/// The base unit for basis points, i.e. how many basis points in 100%.
const BPS_BASE: u32 = 10_000;

pub fn to_domain(id: liquidity::Id, pool: ConstantProductOrder) -> Option<liquidity::Liquidity> {
    assert_eq!(
        (pool.fee.numer() * BPS_BASE) % pool.fee.denom(),
        0,
        "invalid Swapr fee ratio; does not have exact BPS representation",
    );

    let bps = (pool.fee.numer() * BPS_BASE) / pool.fee.denom();
    let fee = swapr::Fee::new(bps)?;
    Some(liquidity::Liquidity {
        id,
        gas: price_estimation::gas::GAS_PER_UNISWAP.into(),
        kind: liquidity::Kind::Swapr(swapr::Pool {
            base: boundary::liquidity::uniswap::v2::to_domain_pool(pool)?,
            fee,
        }),
    })
}

pub async fn collector(
    eth: &Ethereum,
    blocks: &CurrentBlockStream,
    config: &infra::liquidity::config::UniswapV2,
) -> Result<Box<dyn LiquidityCollecting>> {
    boundary::liquidity::uniswap::v2::collector_with_reader(
        eth,
        blocks,
        config,
        |web3, pair_provider| {
            SwaprPoolReader(DefaultPoolReader {
                web3,
                pair_provider,
            })
        },
    )
    .await
}
