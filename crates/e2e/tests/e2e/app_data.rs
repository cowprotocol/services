use {
    app_data::AppDataHash,
    e2e::{setup::*, tx},
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    reqwest::StatusCode,
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::str::FromStr,
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
    let [solver] = onchain.make_solvers(to_wei(1)).await;
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

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Unknown hashes are not accepted.
    let order0 = create_order(OrderCreationAppData::Hash {
        hash: AppDataHash([1; 32]),
    });
    let err = services
        .get_app_data(AppDataHash([1; 32]))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND);

    assert!(services.create_order(&order0).await.is_err());

    // hash matches
    let app_data = "{}";
    let app_data_hash = AppDataHash(app_data_hash::hash_full_app_data(app_data.as_bytes()));
    let order1 = create_order(OrderCreationAppData::Both {
        full: app_data.to_string(),
        expected: app_data_hash,
    });
    let uid = services.create_order(&order1).await.unwrap();
    let order1_ = services.get_order(&uid).await.unwrap();
    assert_eq!(order1_.data.app_data, app_data_hash);
    assert_eq!(order1_.metadata.full_app_data, Some(app_data.to_string()));

    let app_data_ = services.get_app_data(app_data_hash).await.unwrap();
    assert_eq!(app_data_, app_data);

    // hash doesn't match
    let order2 = create_order(OrderCreationAppData::Both {
        full: r#"{"hello":"world"}"#.to_string(),
        expected: app_data_hash,
    });
    let err = services.create_order(&order2).await.unwrap_err();
    dbg!(err);

    // no full app data specified but hash matches existing hash in database from
    // order1
    let order3 = create_order(OrderCreationAppData::Hash {
        hash: app_data_hash,
    });
    services
        .submit_quote(&OrderQuoteRequest {
            sell_token: order3.sell_token,
            buy_token: order3.buy_token,
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee {
                    value: order3.sell_amount.try_into().unwrap(),
                },
            },
            app_data: OrderCreationAppData::Hash {
                hash: app_data_hash,
            },
            ..Default::default()
        })
        .await
        .unwrap();

    let uid = services.create_order(&order3).await.unwrap();
    let order3_ = services.get_order(&uid).await.unwrap();
    assert_eq!(order3_.data.app_data, app_data_hash);
    // Contrast this with order0, which doesn't have full app data.
    assert_eq!(order3_.metadata.full_app_data.as_deref(), Some(app_data));

    // invalid app data
    let invalid_app_data = r#"{"metadata":"invalid"}"#;
    let order4 = create_order(OrderCreationAppData::Full {
        full: invalid_app_data.to_string(),
    });
    let err = services.create_order(&order4).await.unwrap_err();
    dbg!(err);

    // pre-register some app-data with the API.
    let pre_app_data = r#"{"pre":"registered"}"#;
    let pre_app_data_hash = AppDataHash(app_data_hash::hash_full_app_data(pre_app_data.as_bytes()));
    let err = services.get_app_data(pre_app_data_hash).await.unwrap_err();
    dbg!(err);

    // not specifying the app data hash will make the backend compute it.
    let response = services.put_app_data(None, pre_app_data).await.unwrap();
    dbg!(&response);
    assert_eq!(AppDataHash::from_str(&response).unwrap(), pre_app_data_hash);
    assert_eq!(
        services.get_app_data(pre_app_data_hash).await.unwrap(),
        pre_app_data
    );

    // creating an order with the pre-registed app-data works.
    let order5 = create_order(OrderCreationAppData::Hash {
        hash: pre_app_data_hash,
    });
    let uid = services.create_order(&order5).await.unwrap();
    let order5_ = services.get_order(&uid).await.unwrap();
    assert_eq!(
        order5_.metadata.full_app_data.as_deref(),
        Some(pre_app_data)
    );

    // pre-registering is idempotent.
    services
        .put_app_data(Some(pre_app_data_hash), pre_app_data)
        .await
        .unwrap();
    assert_eq!(
        services.get_app_data(pre_app_data_hash).await.unwrap(),
        pre_app_data
    );

    // pre-registering invalid app-data fails.
    let err = services
        .put_app_data(
            Some(AppDataHash(app_data_hash::hash_full_app_data(
                invalid_app_data.as_bytes(),
            ))),
            invalid_app_data,
        )
        .await
        .unwrap_err();
    dbg!(err);
}
