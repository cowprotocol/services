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
};

/// Seconds a `validFrom` is set into the future; large enough to observe the
/// order held out of the auction across several autopilot cycles.
const GATE_SECS: u32 = 6;

#[tokio::test]
#[ignore]
async fn local_node_valid_from() {
    run_test(valid_from_test).await;
}

/// `validFrom` (app-data, unix seconds) holds an order out of the batch auction
/// until `now >= validFrom`. Verified for a regular EIP-712 order and for an
/// on-chain ethflow order, whose `validFrom` is backfilled from the app-data.
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

    // Regular EIP-712 order gated by a future validFrom in its app-data.
    let valid_from = model::time::now_in_epoch_seconds() + GATE_SECS;
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: format!(r#"{{"metadata":{{"validFrom":{valid_from}}}}}"#),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();
    settles_only_after_valid_from(&onchain, &services, uid, valid_from).await;

    // On-chain ethflow order: the app-data only exists behind its hash
    // on-chain, so validFrom is backfilled while indexing the order.
    let ethflow_valid_from = model::time::now_in_epoch_seconds() + GATE_SECS;
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
        validTo: model::time::now_in_epoch_seconds() + 3600,
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
    settles_only_after_valid_from(&onchain, &services, ethflow_uid, ethflow_valid_from).await;
}

/// Asserts the order never settles while `now < valid_from`, then settles once
/// `validFrom` has passed.
async fn settles_only_after_valid_from(
    onchain: &OnchainComponents,
    services: &Services<'_>,
    uid: OrderUid,
    valid_from: u32,
) {
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let Ok(order) = services.get_order(&uid).await else {
            return false;
        };
        if model::time::now_in_epoch_seconds() < valid_from {
            assert_eq!(
                order.metadata.status,
                OrderStatus::Open,
                "order settled before validFrom",
            );
            false
        } else {
            order.metadata.status == OrderStatus::Fulfilled
        }
    })
    .await
    .unwrap();
}
