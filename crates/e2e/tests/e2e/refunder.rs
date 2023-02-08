use {
    crate::{
        deploy::Contracts,
        eth_flow::{EthFlowOrderOnchainStatus, ExtendedEthFlowOrder, ORDERS_ENDPOINT},
        local_node::{AccountAssigner, TestNodeApi},
        onchain_components::{
            deploy_token_with_weth_uniswap_pool,
            to_wei,
            MintableToken,
            WethPoolConfig,
        },
        services::{OrderbookServices, API_HOST},
    },
    anyhow::Result,
    chrono::{DateTime, NaiveDateTime, Utc},
    ethcontract::{H160, H256, U256},
    model::{
        order::Order,
        quote::{
            OrderQuoteRequest,
            OrderQuoteResponse,
            OrderQuoteSide,
            QuoteSigningScheme,
            Validity,
        },
    },
    refunder::refund_service::RefundService,
    reqwest::Client,
    shared::{
        current_block::timestamp_of_current_block_in_seconds,
        ethrpc::Web3,
        http_client::HttpClientFactory,
        maintenance::Maintaining,
    },
    sqlx::PgPool,
};

const QUOTING_ENDPOINT: &str = "/api/v1/quote/";

#[tokio::test]
#[ignore]
async fn local_node_refunder_tx() {
    crate::local_node::test(refunder_tx).await;
}

async fn refunder_tx(web3: Web3) {
    shared::tracing::initialize_reentrant("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let mut accounts = AccountAssigner::new(&web3).await;
    let user = accounts.assign_free_account();
    let refunder_account = accounts.assign_free_account();

    // Create & mint tokens to trade, pools for fee connections
    let MintableToken {
        contract: token, ..
    } = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(100_000),
            weth_amount: to_wei(100_000),
        },
    )
    .await;

    let services = OrderbookServices::new(&web3, &contracts, true).await;
    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    // Get quote id for order placement
    let buy_token = token.address();
    let receiver = Some(H160([42; 20]));
    let sell_amount = U256::from("3000000000000000");

    let quote = OrderQuoteRequest {
        from: contracts.ethflow.address(),
        sell_token: contracts.weth.address(),
        buy_token,
        receiver,
        validity: Validity::For(u32::MAX),
        signing_scheme: QuoteSigningScheme::Eip1271 {
            onchain_order: true,
            verification_gas_limit: 0,
        },
        side: OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::AfterFee { value: sell_amount },
        },
        ..Default::default()
    };
    let quoting = client
        .post(&format!("{API_HOST}{QUOTING_ENDPOINT}"))
        .json(&quote)
        .send()
        .await
        .unwrap();
    assert_eq!(quoting.status(), 200);
    let quote_response = quoting.json::<OrderQuoteResponse>().await.unwrap();

    let validity_duration = 600;
    let valid_to = timestamp_of_current_block_in_seconds(&web3).await.unwrap() + validity_duration;
    // Accounting for slippage is necesary for the order to be picked up by the
    // refunder
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(9999);

    ethflow_order
        .mine_order_creation(&user, &contracts.ethflow)
        .await;

    // Run autopilot indexing loop
    services.maintenance.run_maintenance().await.unwrap();

    let time_after_expiration = valid_to as i64 + 60;
    web3.api::<TestNodeApi<_>>()
        .set_next_block_timestamp(&DateTime::from_utc(
            NaiveDateTime::from_timestamp(time_after_expiration, 0),
            Utc,
        ))
        .await
        .expect("Must be able to set block timestamp");
    // mine next block to push time forward
    web3.api::<TestNodeApi<_>>()
        .mine_pending_block()
        .await
        .expect("Unable to mine next block");

    // Create the refund service and execute the refund tx
    let pg_pool = PgPool::connect_lazy("postgresql://").expect("failed to create database");
    let mut refunder = RefundService::new(
        pg_pool,
        web3,
        contracts.ethflow.clone(),
        validity_duration as i64 / 2,
        10u64,
        refunder_account,
    );

    assert_ne!(
        ethflow_order.status(&contracts).await,
        EthFlowOrderOnchainStatus::Invalidated
    );

    refunder.try_to_refund_all_eligble_orders().await.unwrap();

    assert_eq!(
        ethflow_order.status(&contracts).await,
        EthFlowOrderOnchainStatus::Invalidated
    );

    // Run autopilot to index refund tx
    services.maintenance.run_maintenance().await.unwrap();

    let tx_hash = get_refund_tx_hash_for_order_uid(&client, &ethflow_order, &contracts)
        .await
        .unwrap();
    assert!(tx_hash.is_some());
}

async fn get_refund_tx_hash_for_order_uid(
    client: &Client,
    ethflow_order: &ExtendedEthFlowOrder,
    contracts: &Contracts,
) -> Result<Option<H256>> {
    let query = client
        .get(&format!(
            r#"{API_HOST}{ORDERS_ENDPOINT}/{}"#,
            ethflow_order.uid(contracts).await,
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), 200);
    let response = query.json::<Order>().await.unwrap();

    Ok(response.metadata.ethflow_data.unwrap().refund_tx_hash)
}
