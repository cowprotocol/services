use {
    e2e::{setup::*, tx},
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_buffers() {
    run_test(onchain_settlement_without_liquidity).await;
}

async fn onchain_settlement_without_liquidity(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(trader.address(), to_wei(100)).await;
    token_b
        .mint(onchain.contracts().gp_settlement.address(), to_wei(5))
        .await;
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(100))
    );

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    let services = Services::new(&onchain).await;
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--trusted-tokens={weth:#x},{token_a:#x},{token_b:#x}",
                    weth = onchain.contracts().weth.address(),
                    token_a = token_a.address(),
                    token_b = token_b.address()
                ),
                format!(
                    "--drivers=test_solver|http://localhost:11088/test_solver|{}",
                    hex::encode(solver.address())
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Place Order
    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(9),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    tracing::info!("waiting for first trade");
    onchain.mint_block().await;
    let trade_happened =
        || async { token_b.balance_of(trader.address()).call().await.unwrap() == order.buy_amount };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check that settlement buffers were traded.
    let settlement_contract_balance = token_b
        .balance_of(onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    // Check that internal buffers were used
    assert!(settlement_contract_balance == 0.into());

    // Same order can trade again with external liquidity
    let order = OrderCreation {
        valid_to: model::time::now_in_epoch_seconds() + 301,
        ..order
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    tracing::info!("waiting for second trade");
    let trade_happened = || async {
        token_b.balance_of(trader.address()).call().await.unwrap() == order.buy_amount * 2
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
}
