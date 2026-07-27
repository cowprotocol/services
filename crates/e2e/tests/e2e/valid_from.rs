use {
    crate::ethflow::ExtendedEthFlowOrder,
    ::alloy::primitives::{Address, U256},
    contracts::CoWSwapEthFlow,
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus, OrderUid},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn local_node_valid_from() {
    run_test(valid_from_test).await;
}

async fn valid_from_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 10u64.eth()).await;
    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let now = model::time::now_in_epoch_seconds();
    let valid_from = now + 3;

    let app_data = format!(r#"{{"metadata":{{"validFrom":{valid_from}}}}}"#);
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: now + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    order_filled_after_valid_from(&onchain, &services, uid, valid_from).await;

    // Now do the same for an ethflow order to verify that `validFrom` is also
    // honored when it comes in via the app data attached to an on-chain order.
    let now = model::time::now_in_epoch_seconds();
    let ethflow_valid_from = now + 3;
    let ethflow_app_data = format!(r#"{{"metadata":{{"validFrom":{ethflow_valid_from}}}}}"#);
    let app_data_hash = services
        .put_app_data(None, &ethflow_app_data)
        .await
        .unwrap();
    let app_data_hash: [u8; 32] = const_hex::decode(&app_data_hash[2..])
        .unwrap()
        .try_into()
        .unwrap();

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    let ethflow_order = ExtendedEthFlowOrder(CoWSwapEthFlow::EthFlowOrder::Data {
        buyToken: *token_b.address(),
        sellAmount: 1u64.eth(),
        buyAmount: U256::ONE,
        validTo: now + 3600,
        partiallyFillable: false,
        quoteId: 0,
        feeAmount: U256::ZERO,
        receiver: Address::from_slice(&[0x43; 20]),
        appData: app_data_hash.into(),
    });
    ethflow_order
        .mine_order_creation(trader.address(), ethflow_contract)
        .await;
    let ethflow_uid = ethflow_order
        .uid(onchain.contracts(), ethflow_contract)
        .await;

    order_filled_after_valid_from(&onchain, &services, ethflow_uid, ethflow_valid_from).await;
}

async fn order_filled_after_valid_from(
    onchain: &OnchainComponents,
    services: &Services<'_>,
    order_uid: OrderUid,
    valid_from: u32,
) {
    tokio::time::timeout(TIMEOUT, async {
        loop {
            onchain.mint_block().await;
            let now_in_unix = model::time::now_in_epoch_seconds();
            let Ok(order) = services.get_order(&order_uid).await else {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            };
            let status = order.metadata.status;
            if now_in_unix < valid_from {
                assert_eq!(status, OrderStatus::Open);
            } else if now_in_unix > valid_from + 1 {
                assert_eq!(status, OrderStatus::Fulfilled);
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap();
}
