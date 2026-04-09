use {
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_uncovered_order() {
    run_test(test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_full_balance_check() {
    run_test(test_full_balance_check).await;
}

/// Tests that a user can already create an order if they only have
/// 1 wei of the sell token and later fund their account to get the
/// order executed.
async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let weth = &onchain.contracts().weth;

    weth.approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    tracing::info!("Placing order with 0 sell tokens");
    let order = OrderCreation {
        sell_token: *weth.address(),
        sell_amount: 2u64.eth(),
        fee_amount: ::alloy::primitives::U256::ZERO,
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    // This order can't be created because we require the trader
    // to have at least 1 wei of sell tokens.
    services.create_order(&order).await.unwrap_err();

    tracing::info!("Placing order with 1 wei of sell_tokens");
    weth.deposit()
        .from(trader.address())
        .value(::alloy::primitives::U256::ONE)
        .send_and_watch()
        .await
        .unwrap();
    // Now that the trader has some funds they are able to create
    // an order (even if it exceeds their current balance).
    services.create_order(&order).await.unwrap();

    tracing::info!("Deposit ETH to make order executable");
    weth.deposit()
        .from(trader.address())
        .value(2u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = weth.balanceOf(trader.address()).call().await.unwrap();
        !balance_after.is_zero()
    })
    .await
    .unwrap();
}

/// Tests that when order is placed with full balance checks, it would not be
/// possible to place it, unless the account has both - sufficient allowance and
/// balance.
///
/// It compares and asserts on the behaviour when the full balance check is
/// disabled and enabled.
async fn test_full_balance_check(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let weth = &onchain.contracts().weth;

    // Initial allowance is enough to pass basic checks, but not enough to execute
    // the order
    weth.approve(onchain.contracts().allowance, 1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Initial balance is enough to pass basic checks, but not enough to execute the
    // order
    weth.deposit()
        .from(trader.address())
        .value(1u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: *weth.address(),
        sell_amount: 2u64.eth(),
        fee_amount: ::alloy::primitives::U256::ZERO,
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        ..Default::default()
    };
    let unchecked_order = order.clone().sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let order = OrderCreation {
        full_balance_check: true,
        // different valid_to for orders to be considered distinct
        valid_to: model::time::now_in_epoch_seconds() + 301,
        ..order
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    // This order can be created because full balance checks are not enabled.
    // The account has 1 WEI of the token and allowance for this amount.
    services.create_order(&unchecked_order).await.unwrap();

    // This order can not be created, because despite the token being transferrable
    // The account does not have enough sell token balance to cover the order.
    assert!(
        services
            .create_order(&order)
            .await
            .unwrap_err()
            .1
            .contains("InsufficientBalance")
    );

    // Add the missing balance for the balance checkE
    weth.deposit()
        .from(trader.address())
        .value(1u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    // This order can not be created, because the account does not have enough
    // sell token allowance.
    assert!(
        services
            .create_order(&order)
            .await
            .unwrap_err()
            .1
            .contains("InsufficientAllowance")
    );

    // Set allowance to the full amount
    weth.approve(onchain.contracts().allowance, 2u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // This order can be created now, because account has correct balance and
    // allowance
    services.create_order(&order).await.unwrap();
}
