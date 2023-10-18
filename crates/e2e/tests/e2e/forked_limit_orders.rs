use {
    contracts::ERC20Mintable,
    e2e::{nodes::forked_node::ForkedNodeApi, setup::*, tx},
    ethcontract::{prelude::U256, H160},
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn forked_node_single_limit_order_mainnet() {
    run_forked_test(
        forked_single_limit_order_test,
        "0x0ab21031124af2165586fbb495d93725a372c227"
            .parse()
            .unwrap(),
        std::env::var("FORK_URL").unwrap(),
    )
    .await;
}

async fn forked_single_limit_order_test(web3: Web3) {
    // begin forking

    let mut onchain = OnchainComponents::at(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let auth_manager = onchain
        .contracts()
        .gp_authenticator
        .manager()
        .call()
        .await
        .unwrap();

    forked_node_api.impersonate(&auth_manager).await.unwrap();

    let [solver] = onchain
        .make_solvers_forked(to_wei(1), ethcontract::Account::Local(auth_manager, None))
        .await;

    // set the trader_a USDC balance to 0xffffffffffffff
    forked_node_api
        .set_storage_at(
            &"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                .parse()
                .unwrap(),
            "0x9b759eea787bae9d56c9286680e0c9074b2a74f24a693d50c695980a8744a6ce",
            "0x00000000000000000000000000000000000000000000000000ffffffffffffff",
        )
        .await
        .unwrap();

    let [trader_a] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = MintableToken {
        contract: ERC20Mintable::at(
            &web3,
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                .parse()
                .unwrap(),
        ),
        minter: ethcontract::Account::Local(H160::zero(), None),
    };

    let token_usdt = MintableToken {
        contract: ERC20Mintable::at(
            &web3,
            "0xdac17f958d2ee523a2206206994597c13d831ec7"
                .parse()
                .unwrap(),
        ),
        minter: ethcontract::Account::Local(H160::zero(), None),
    };

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token_usdc.approve(onchain.contracts().allowance, to_wei(10))
    );

    // Place Orders
    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![]);
    services.start_api(vec![]).await;

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei(10),
        buy_token: token_usdt.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(
        limit_order.metadata.class,
        OrderClass::Limit(Default::default())
    );

    // Drive solution
    tracing::info!("Waiting for trade.");
    let balance_before = token_usdt
        .balance_of(trader_a.address())
        .call()
        .await
        .unwrap();
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    services.start_old_driver(solver.private_key(), vec![]);
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let balance_after = token_usdt
        .balance_of(trader_a.address())
        .call()
        .await
        .unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= to_wei(5));
}
