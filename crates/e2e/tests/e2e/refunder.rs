use {
    crate::{
        eth_flow::{EthFlowOrderOnchainStatus, ExtendedEthFlowOrder},
        local_node::TestNodeApi,
        onchain_components::{
            deploy_token_with_weth_uniswap_pool,
            to_wei,
            MintableToken,
            WethPoolConfig,
        },
        services::{wait_for_condition, API_HOST},
    },
    chrono::{DateTime, NaiveDateTime, Utc},
    ethcontract::{transaction::TransactionBuilder, Account, PrivateKey, H160, U256},
    hex_literal::hex,
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
    shared::{current_block::timestamp_of_current_block_in_seconds, ethrpc::Web3},
    sqlx::PgPool,
    std::time::Duration,
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

    let user_pk = hex!("0000000000000000000000000000000000000000000000000000000000000001");
    let user = Account::Offline(PrivateKey::from_raw(user_pk).unwrap(), None);
    let refunder_pk = hex!("0000000000000000000000000000000000000000000000000000000000000002");
    let refunder = Account::Offline(PrivateKey::from_raw(refunder_pk).unwrap(), None);

    for account in [&user, &refunder] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(10))
            .to(account.address())
            .send()
            .await
            .unwrap();
    }

    // Create & mint tokens to trade, pools for fee connections
    let MintableToken {
        contract: token, ..
    } = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;

    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    // Get quote id for order placement
    let buy_token = token.address();
    let receiver = Some(H160([42; 20]));
    let sell_amount = U256::from("3000000000000000");

    let quote = OrderQuoteRequest {
        from: contracts.ethflow.address(),
        sell_token: contracts.weth.address(),
        buy_token,
        receiver,
        validity: Validity::For(3600),
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
    // Accounting for slippage is necessary for the order to be picked up by the
    // refunder
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(9999);

    ethflow_order
        .mine_order_creation(&user, &contracts.ethflow)
        .await;

    let get_order = || async {
        client
            .get(format!(
                "{API_HOST}/api/v1/orders/{}",
                ethflow_order.uid(&contracts).await
            ))
            .send()
            .await
            .unwrap()
    };

    tracing::info!("Waiting for order to be indexed.");
    let order_exists = || async {
        let response = get_order().await;
        response.status().is_success()
    };
    wait_for_condition(Duration::from_secs(10), order_exists)
        .await
        .unwrap();

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
        refunder,
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

    tracing::info!("Waiting for autopilot to index refund tx hash.");
    let has_tx_hash = || async {
        let order = get_order().await.json::<Order>().await.unwrap();
        order
            .metadata
            .ethflow_data
            .unwrap()
            .refund_tx_hash
            .is_some()
    };
    wait_for_condition(Duration::from_secs(10), has_tx_hash)
        .await
        .unwrap();
}
