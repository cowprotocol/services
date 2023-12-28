use {
    crate::{
        domain::liquidity::{self, zeroex},
        infra::{self, Ethereum},
    },
    ethrpc::current_block::CurrentBlockStream,
    shared::{
        http_client::HttpClientFactory,
        price_estimation::gas::GAS_PER_ZEROEX_ORDER,
        zeroex_api::DefaultZeroExApi,
    },
    solver::{
        liquidity::{zeroex::ZeroExLiquidity, LimitOrder},
        liquidity_collector::LiquidityCollecting,
    },
    std::sync::Arc,
};

pub fn to_domain(id: liquidity::Id, pool: LimitOrder) -> anyhow::Result<liquidity::Liquidity> {
    Ok(liquidity::Liquidity {
        id,
        gas: GAS_PER_ZEROEX_ORDER.into(),
        kind: liquidity::Kind::ZeroEx(zeroex::LimitOrder::new(pool)?),
    })
}

pub async fn collector(
    eth: &Ethereum,
    blocks: CurrentBlockStream,
    config: &infra::liquidity::config::ZeroEx,
) -> anyhow::Result<Box<dyn LiquidityCollecting>> {
    let settlement = eth.contracts().settlement().clone();
    let web3 = settlement.raw_instance().web3().clone();
    let contract = contracts::IZeroEx::deployed(&web3).await?;
    let http_client_factory = &HttpClientFactory::new(&shared::http_client::Arguments {
        http_timeout: config.http_timeout,
    });
    let api = Arc::new(DefaultZeroExApi::new(
        http_client_factory,
        config.base_url.clone(),
        config.api_key.clone(),
        blocks,
    )?);
    Ok(Box::new(ZeroExLiquidity::new(
        web3, api, contract, settlement,
    )))
}
