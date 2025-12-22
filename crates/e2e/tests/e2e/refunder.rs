use {
    crate::ethflow::{EthFlowOrderOnchainStatus, ExtendedEthFlowOrder},
    ::alloy::{primitives::Address, providers::ext::AnvilApi},
    chrono::{TimeZone, Utc},
    e2e::setup::*,
    ethrpc::{Web3, alloy::EvmProviderExt, block_stream::timestamp_of_current_block_in_seconds},
    model::quote::{OrderQuoteRequest, OrderQuoteSide, QuoteSigningScheme, Validity},
    number::{nonzero::NonZeroU256, units::EthUnit},
    refunder::refund_service::RefundService,
    sqlx::PgPool,
};

#[tokio::test]
#[ignore]
async fn local_node_refunder_tx() {
    run_test(refunder_tx).await;
}

async fn refunder_tx(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [user, refunder] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Get quote id for order placement
    let buy_token = *token.address();
    let receiver = Some(Address::repeat_byte(42));
    let sell_amount = NonZeroU256::try_from(3000000000000000u128).unwrap();

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    let quote = OrderQuoteRequest {
        from: *ethflow_contract.address(),
        sell_token: *onchain.contracts().weth.address(),
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
    let valid_to = timestamp_of_current_block_in_seconds(&web3.alloy)
        .await
        .unwrap()
        + validity_duration;
    // Accounting for slippage is necessary for the order to be picked up by the
    // refunder
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(9999);

    // create second ethflow order using secondary ethflow contract
    let ethflow_contract_2 = onchain.contracts().ethflows.get(1).unwrap();

    let quote = OrderQuoteRequest {
        from: *ethflow_contract_2.address(),
        sell_token: *onchain.contracts().weth.address(),
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
    let ethflow_order_2 =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(9999);

    ethflow_order
        .mine_order_creation(user.address(), ethflow_contract)
        .await;
    ethflow_order_2
        .mine_order_creation(user.address(), ethflow_contract_2)
        .await;

    let order_id = ethflow_order
        .uid(onchain.contracts(), ethflow_contract)
        .await;
    let order_id_2 = ethflow_order_2
        .uid(onchain.contracts(), ethflow_contract_2)
        .await;

    tracing::info!("Waiting for orders to be indexed.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services.get_order(&order_id).await.is_ok() && services.get_order(&order_id_2).await.is_ok()
    })
    .await
    .unwrap();

    let time_after_expiration = valid_to as i64 + 60;
    web3.alloy
        .evm_set_next_block_timestamp(
            u64::try_from(
                Utc.timestamp_millis_opt(time_after_expiration * 1_000)
                    .unwrap()
                    .timestamp(),
            )
            .expect("timestamp should be positive"),
        )
        .await
        .expect("Must be able to set block timestamp");

    // mine next block to push time forward
    web3.alloy
        .evm_mine(None)
        .await
        .expect("Unable to mine next block");

    // Create the refund service and execute the refund tx
    let pg_pool = PgPool::connect_lazy("postgresql://").expect("failed to create database");
    let mut refunder = RefundService::new(
        pg_pool,
        web3,
        vec![ethflow_contract.clone(), ethflow_contract_2.clone()],
        validity_duration as i64 / 2,
        10i64,
        Box::new(refunder.signer),
        2_000_000_000_000, // max_gas_price: 2000 Gwei
        30_000_000_000,    // start_priority_fee_tip: 30 Gwei
    );

    assert_ne!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
            .await,
        EthFlowOrderOnchainStatus::Invalidated
    );
    assert_ne!(
        ethflow_order_2
            .status(onchain.contracts(), ethflow_contract_2)
            .await,
        EthFlowOrderOnchainStatus::Invalidated
    );

    refunder.try_to_refund_all_eligble_orders().await.unwrap();

    assert_eq!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
            .await,
        EthFlowOrderOnchainStatus::Invalidated
    );
    assert_eq!(
        ethflow_order_2
            .status(onchain.contracts(), ethflow_contract_2)
            .await,
        EthFlowOrderOnchainStatus::Invalidated
    );

    tracing::info!("Waiting for autopilot to index refund tx hash.");
    for order in &[order_id, order_id_2] {
        let has_tx_hash = || async {
            onchain.mint_block().await;
            services
                .get_order(order)
                .await
                .unwrap()
                .metadata
                .ethflow_data
                .unwrap()
                .refund_tx_hash
                .is_some()
        };
        wait_for_condition(TIMEOUT, has_tx_hash).await.unwrap();
    }
}
