use {
    e2e::{
        setup::{safe::Safe, *},
        tx,
    },
    ethcontract::{prelude::U256, H160},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_order_creation_checks_metadata_signer() {
    run_test(order_creation_checks_metadata_signer).await;
}

async fn order_creation_checks_metadata_signer(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader, adversary, safe_owner] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    token_a.mint(trader.address(), to_wei(10)).await;
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(10))
    );
    token_a.mint(adversary.address(), to_wei(10)).await;
    tx!(
        adversary.account(),
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
        };
        // Adjust valid to make sure we get unique UIDs.
        valid_to += 1;
        order
    };
    let sign = |order_creation: OrderCreation, signer: &TestAccount| {
        order_creation.sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(signer.private_key()).unwrap()),
        )
    };

    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    // Rejected: app data with different signer.
    let full_app_data = full_app_data_with_signer(adversary.address());
    let order1 = sign(create_order(full_app_data), &trader);
    let err = services.create_order(&order1).await.unwrap_err();
    assert!(dbg!(err).1.contains("WrongOwner"));

    // Accepted: app data with correct signer.
    let full_app_data = full_app_data_with_signer(trader.address());
    let order2 = sign(create_order(full_app_data.clone()), &trader);
    let uid = services.create_order(&order2).await.unwrap();
    assert!(matches!(services.get_order(&uid).await, Ok(..)));
    let app_data_hash = full_app_data.hash();

    assert!(matches!(services.get_app_data(app_data_hash).await, Ok(..)));
    // Rejected: different implicit signer when app data isn't available.
    let order3 = sign(
        create_order(OrderCreationAppData::Hash {
            hash: app_data_hash,
        }),
        &adversary,
    );
    let err = services.create_order(&order3).await.unwrap_err();
    assert!(dbg!(err).1.contains("WrongOwner"));

    // EIP-1271

    let safe = Safe::deploy(safe_owner.clone(), &web3).await;
    token_a.mint(safe.address(), to_wei(10)).await;
    safe.exec_call(token_a.approve(onchain.contracts().allowance, to_wei(10)))
        .await;

    // Accepted: owner retrieved from app data.
    let full_app_data = full_app_data_with_signer(safe.address());
    let mut order4 = create_order(full_app_data);
    safe.sign_order(&mut order4, &onchain);
    assert!(matches!(services.create_order(&order4).await, Ok(..)));

    // Rejected: from and signer are inconsistent.
    let full_app_data = full_app_data_with_signer(adversary.address());
    let mut order5 = create_order(full_app_data);
    order5.from = Some(safe.address());
    safe.sign_order(&mut order5, &onchain);
    let err = services.create_order(&order5).await.unwrap_err();
    assert!(err.1.contains("AppdataFromMismatch"));
}

fn full_app_data_with_signer(signer: H160) -> OrderCreationAppData {
    let app_data = format!("{{\"metadata\": {{\"signer\": \"{:?}\"}}}}", signer);
    OrderCreationAppData::Full {
        full: app_data.to_string(),
    }
}
