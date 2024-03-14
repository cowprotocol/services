use {
    crate::{infra, infra::Ethereum},
    ethrpc::current_block::CurrentBlockStream,
    shared::{http_client::HttpClientFactory, zeroex_api::DefaultZeroExApi},
    solver::{liquidity::zeroex::ZeroExLiquidity, liquidity_collector::LiquidityCollecting},
    std::sync::Arc,
};

pub async fn collector(
    eth: &Ethereum,
    blocks: CurrentBlockStream,
    config: &infra::liquidity::config::ZeroEx,
) -> anyhow::Result<Box<dyn LiquidityCollecting>> {
    let eth = eth.with_metric_label("zeroex".into());
    let settlement = eth.contracts().settlement().clone();
    let web3 = settlement.raw_instance().web3().clone();
    let contract = contracts::IZeroEx::deployed(&web3).await?;
    let http_client_factory = &HttpClientFactory::new(&shared::http_client::Arguments {
        http_timeout: config.http_timeout,
    });
    let api = Arc::new(DefaultZeroExApi::new(
        http_client_factory.builder(),
        config.base_url.clone(),
        config.api_key.clone(),
        blocks,
    )?);
    Ok(Box::new(ZeroExLiquidity::new(
        web3, api, contract, settlement,
    )))
}
