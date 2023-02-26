use {
    crate::{
        eth_flow::{EthFlowOrderOnchainStatus, ExtendedEthFlowOrder},
        helpers::*,
        local_node::TestNodeApi,
    },
    chrono::{DateTime, NaiveDateTime, Utc},
    ethcontract::{H160, U256},
    model::{
        order::Order,
        quote::{OrderQuoteRequest, OrderQuoteSide, QuoteSigningScheme, Validity},
    },
    refunder::refund_service::RefundService,
    shared::{current_block::timestamp_of_current_block_in_seconds, ethrpc::Web3},
    sqlx::PgPool,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn local_node_refunder_tx() {
    crate::local_node::test(refunder_tx).await;
}

async fn refunder_tx(web3: Web3) {
    init().await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [user, refunder] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![]);
    services.start_api(vec![]).await;

    let client = reqwest::Client::default();

    // Get quote id for order placement
    let buy_token = token.address();
    let receiver = Some(H160([42; 20]));
    let sell_amount = U256::from("3000000000000000");

    let quote = OrderQuoteRequest {
        from: onchain.contracts().ethflow.address(),
        sell_token: onchain.contracts().weth.address(),
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
    let quote_response = services.submit_quote(&quote).await.unwrap();

    let validity_duration = 600;
    let valid_to = timestamp_of_current_block_in_seconds(&web3).await.unwrap() + validity_duration;
    // Accounting for slippage is necessary for the order to be picked up by the
    // refunder
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(9999);

    ethflow_order
        .mine_order_creation(user.account(), &onchain.contracts().ethflow)
        .await;

    let get_order = || async {
        client
            .get(format!(
                "{API_HOST}/api/v1/orders/{}",
                ethflow_order.uid(onchain.contracts()).await
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
        onchain.contracts().ethflow.clone(),
        validity_duration as i64 / 2,
        10u64,
        refunder.account().clone(),
    );

    assert_ne!(
        ethflow_order.status(onchain.contracts()).await,
        EthFlowOrderOnchainStatus::Invalidated
    );

    refunder.try_to_refund_all_eligble_orders().await.unwrap();

    assert_eq!(
        ethflow_order.status(onchain.contracts()).await,
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
