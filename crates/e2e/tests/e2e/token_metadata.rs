use {
    bigdecimal::Zero,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_token_metadata() {
    run_test(token_metadata).await;
}

#[tokio::test]
#[ignore]
async fn local_node_token_metadata_no_trade() {
    run_test(token_metadata_no_trade).await;
}

/// Test that the token metadata endpoint returns correct metadata after a trade
async fn token_metadata(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
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

    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let uid = services.create_order(&order).await.unwrap();

    // Wait for order to be settled
    onchain.mint_block().await;
    let settlement_finished = || async {
        let order = services.get_order(&uid).await.unwrap();
        !order.metadata.executed_buy_amount.is_zero()
    };
    wait_for_condition(TIMEOUT, settlement_finished)
        .await
        .unwrap();

    // Get metadata for token_a (which was traded)
    let metadata = services
        .get_token_metadata(token_a.address())
        .await
        .unwrap();

    // After a trade, the token should have metadata with valid values
    let first_trade_block = metadata
        .first_trade_block
        .expect("Token should have first_trade_block after being traded");
    assert!(
        first_trade_block > 0,
        "First trade block should be greater than 0, got {first_trade_block}"
    );

    let native_price = metadata
        .native_price
        .expect("Token should have native_price after being traded");
    assert!(
        !native_price.is_zero(),
        "Native price should be non-zero after trading"
    );
}

/// Test that the token metadata endpoint returns None values for tokens with no
/// trades
async fn token_metadata_no_trade(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    // Deploy a token but don't trade it
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Get metadata for a token that has never been traded
    let metadata = services.get_token_metadata(token.address()).await.unwrap();

    // Token with no trades should have None for first_trade_block
    assert!(
        metadata.first_trade_block.is_none(),
        "Token with no trades should have None for first_trade_block"
    );
    // Native price might still exist from the pool
    // So we don't assert on it
}
