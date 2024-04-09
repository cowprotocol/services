use {
    crate::{
        domain::liquidity::{
            self,
            zeroex::{self, Order, ZeroExSignature},
        },
        infra::{self, Ethereum},
    },
    anyhow::anyhow,
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

pub fn to_domain(
    id: liquidity::Id,
    limit_order: LimitOrder,
) -> anyhow::Result<liquidity::Liquidity> {
    // `order` and `contract` should be provided somehow through the `LimitOrder`
    // struct. Currently, it's not possible to add 0x-specific fields right to
    // the `solver::LimitOrder` since it's used with different settlement
    // handlers. One of the options to address it: to use a separate
    // `solver::Liquidity` enum value for 0x liquidity.
    let handler = limit_order
        .settlement_handling
        .as_any()
        .downcast_ref::<solver::liquidity::zeroex::OrderSettlementHandler>()
        .ok_or(anyhow!("not a zeroex::OrderSettlementHandler"))?
        .clone();

    let signature = ZeroExSignature {
        r: handler.order.signature.r,
        s: handler.order.signature.s,
        v: handler.order.signature.v,
        signature_type: handler.order.signature.signature_type,
    };

    let order = Order {
        maker: handler.order.maker,
        taker: handler.order.taker,
        sender: handler.order.sender,
        maker_token: handler.order.maker_token,
        taker_token: handler.order.taker_token,
        maker_amount: handler.order.maker_amount,
        taker_amount: handler.order.taker_amount,
        taker_token_fee_amount: handler.order.taker_token_fee_amount,
        fee_recipient: handler.order.fee_recipient,
        pool: handler.order.pool,
        expiry: handler.order.expiry,
        salt: handler.order.salt,
        signature,
    };

    let domain = zeroex::LimitOrder {
        order,
        zeroex: handler.zeroex.clone(),
    };

    Ok(liquidity::Liquidity {
        id,
        gas: GAS_PER_ZEROEX_ORDER.into(),
        kind: liquidity::Kind::ZeroEx(domain),
    })
}

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
