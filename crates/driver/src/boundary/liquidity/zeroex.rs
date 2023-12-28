use {
    crate::{
        domain::liquidity::{
            self,
            zeroex::{self, Order},
        },
        infra::{self, Ethereum},
    },
    anyhow::anyhow,
    ethrpc::current_block::CurrentBlockStream,
    model::order::OrderKind,
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
    let handler = pool
        .settlement_handling
        .as_any()
        .downcast_ref::<solver::liquidity::zeroex::OrderSettlementHandler>()
        .ok_or(anyhow!("not a zeroex::OrderSettlementHandler"))?
        .clone();

    let full_execution_amount = match pool.kind {
        OrderKind::Sell => pool.sell_amount,
        OrderKind::Buy => pool.buy_amount,
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
        signature_type: handler.order.signature.signature_type,
        signature_r: handler.order.signature.r,
        signature_s: handler.order.signature.s,
        signature_v: handler.order.signature.v,
    };

    let domain = zeroex::LimitOrder {
        sell_token: pool.sell_token,
        buy_token: pool.buy_token,
        sell_amount: pool.sell_amount,
        buy_amount: pool.buy_amount,
        order,
        full_execution_amount,
        zeroex: handler.zeroex,
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
