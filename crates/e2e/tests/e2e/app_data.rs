use {
    crate::setup::*,
    ethcontract::prelude::U256,
    model::{
        app_id::AppDataHash,
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_app_data() {
    run_test(app_data).await;
}

// Test that orders can be placed with the new app data format.
async fn app_data(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    token_a.mint(trader.address(), to_wei(10)).await;
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(10))
    );

    let mut valid_to: u32 = model::time::now_in_epoch_seconds() + 300;
    let mut create_order = |app_data| {
        let order = OrderCreation {
            app_data,
            sell_token: token_a.address(),
            sell_amount: to_wei(2),
            fee_amount: to_wei(1),
            buy_token: token_b.address(),
            buy_amount: to_wei(1),
            valid_to,
            kind: OrderKind::Sell,
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        );
        // Adjust valid to make sure we get unique UIDs.
        valid_to += 1;
        order
    };

    let services = Services::new(onchain.contracts()).await;
    services.start_api(vec![]).await;

    // Temporarily custom hash are still accepted.
    let order0 = create_order(OrderCreationAppData::Hash {
        hash: AppDataHash([1; 32]),
    });
    services.create_order(&order0).await.unwrap();

    // hash matches
    let app_data = "{}";
    let app_data_hash = app_data_hash::hash_full_app_data(app_data.as_bytes());
    let order1 = create_order(OrderCreationAppData::Both {
        full: app_data.to_string(),
        expected: AppDataHash(app_data_hash),
    });
    services.create_order(&order1).await.unwrap();

    // hash doesn't match
    let order2 = create_order(OrderCreationAppData::Both {
        full: r#"{"hello":"world"}"#.to_string(),
        expected: AppDataHash(app_data_hash),
    });
    let err = services.create_order(&order2).await.unwrap_err();
    dbg!(err);
}
