use {
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
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

    tokio::time::timeout(TIMEOUT, async {
        loop {
            onchain.mint_block().await;
            let status = services.get_order(&uid).await.unwrap().metadata.status;
            let now_in_unix = model::time::now_in_epoch_seconds();
            if now_in_unix < valid_from {
                assert_eq!(status, OrderStatus::Open);
            } else if now_in_unix > valid_from + 1 {
                assert_eq!(status, OrderStatus::Fulfilled);
                break;
            } else {
                // during the time [valid_from..=valid_from + 1] we don't assert
                // anything about the order status so that race conditions don't
                // cause assertions to fail
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap();
}
