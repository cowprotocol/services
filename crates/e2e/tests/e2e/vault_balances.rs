use {
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind, SellTokenSource},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_vault_balances() {
    run_test(vault_balances).await;
}

async fn vault_balances(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 10u64.eth()).await;

    // Approve GPv2 for trading

    token
        .approve(*onchain.contracts().balancer_vault.address(), 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .balancer_vault
        .setRelayerApproval(trader.address(), onchain.contracts().allowance, true)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Place Orders
    let order = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: *token.address(),
        sell_amount: 10u64.eth(),
        sell_token_balance: SellTokenSource::External,
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 8u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();
    onchain.mint_block().await;
    let balance_before = onchain
        .contracts()
        .weth
        .balanceOf(trader.address())
        .call()
        .await
        .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let token_balance = token
            .balanceOf(trader.address())
            .call()
            .await
            .expect("Couldn't fetch token balance");

        let weth_balance_after = onchain
            .contracts()
            .weth
            .balanceOf(trader.address())
            .call()
            .await
            .unwrap();

        token_balance.is_zero() && weth_balance_after.saturating_sub(balance_before) >= 8u64.eth()
    })
    .await
    .unwrap();
}
