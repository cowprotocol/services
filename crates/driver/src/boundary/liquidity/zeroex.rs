use {
    crate::{
        domain::{
            eth,
            liquidity::{self, zeroex},
        },
        infra::{self, Ethereum},
    },
    anyhow::anyhow,
    ethcontract::Bytes,
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

pub fn to_interaction(limit_order: &zeroex::LimitOrder) -> anyhow::Result<eth::Interaction> {
    let method = limit_order.zeroex.fill_or_kill_limit_order(
        (
            limit_order.order.maker_token,
            limit_order.order.taker_token,
            limit_order.order.maker_amount,
            limit_order.order.taker_amount,
            limit_order.order.taker_token_fee_amount,
            limit_order.order.maker,
            limit_order.order.taker,
            limit_order.order.sender,
            limit_order.order.fee_recipient,
            Bytes(limit_order.order.pool.0),
            limit_order.order.expiry,
            limit_order.order.salt,
        ),
        (
            limit_order.order.signature.signature_type,
            limit_order.order.signature.v,
            Bytes(limit_order.order.signature.r.0),
            Bytes(limit_order.order.signature.s.0),
        ),
        limit_order.full_execution_amount.as_u128(),
    );
    let calldata = method.tx.data.ok_or(anyhow!("no calldata"))?;

    Ok(eth::Interaction {
        target: limit_order.zeroex.address().into(),
        value: 0.into(),
        call_data: calldata.0.into(),
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
