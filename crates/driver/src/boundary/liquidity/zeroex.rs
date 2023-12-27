use {
    crate::{
        domain::{
            eth,
            liquidity::{self, zeroex},
        },
        infra::{self, Ethereum},
    },
    ethrpc::current_block::CurrentBlockStream,
    shared::{
        http_client::HttpClientFactory,
        http_solver::model::InternalizationStrategy,
        price_estimation::gas::GAS_PER_ZEROEX_ORDER,
        zeroex_api::DefaultZeroExApi,
    },
    solver::{
        liquidity::{zeroex::ZeroExLiquidity, LimitOrder, LimitOrderExecution},
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    std::sync::Arc,
};

pub fn to_domain(id: liquidity::Id, pool: LimitOrder) -> anyhow::Result<liquidity::Liquidity> {
    Ok(liquidity::Liquidity {
        id,
        gas: GAS_PER_ZEROEX_ORDER.into(),
        kind: liquidity::Kind::ZeroEx(zeroex::LimitOrder::new(pool)),
    })
}

pub fn to_interaction(
    pool: &zeroex::LimitOrder,
    _input: &liquidity::MaxInput,
    _output: &liquidity::ExactOutput,
    _receiver: &eth::Address,
) -> anyhow::Result<eth::Interaction> {
    let mut encoder = SettlementEncoder::new(Default::default());
    let execution =
        LimitOrderExecution::new(pool.inner.full_execution_amount(), pool.inner.scoring_fee);

    pool.inner
        .settlement_handling
        .clone()
        .encode(execution, &mut encoder)?;

    let [_, interactions, _] = encoder
        .finish(InternalizationStrategy::EncodeAllInteractions)
        .interactions;

    let (target, value, call_data) = interactions[1].clone();
    Ok(eth::Interaction {
        target: target.into(),
        value: value.into(),
        call_data: call_data.0.into(),
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
