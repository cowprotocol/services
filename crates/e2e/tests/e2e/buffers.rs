use {
    ::alloy::primitives::U256,
    autopilot::config::{Configuration, trusted_tokens::TrustedTokensConfig},
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
async fn local_node_buffers() {
    run_test(onchain_settlement_without_liquidity).await;
}

async fn onchain_settlement_without_liquidity(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(trader.address(), 100u64.eth()).await;
    token_b
        .mint(*onchain.contracts().gp_settlement.address(), 5u64.eth())
        .await;
    token_a.mint(solver.address(), 1000u64.eth()).await;
    token_b.mint(solver.address(), 1000u64.eth()).await;

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
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
    let (_config_file, config_arg) = Configuration {
        trusted_tokens: TrustedTokensConfig {
            tokens: vec![
                *onchain.contracts().weth.address(),
                *token_a.address(),
                *token_b.address(),
            ],
            ..Default::default()
        },
        ..Configuration::test("test_solver", solver.address())
    }
    .to_cli_args();

    services
        .start_autopilot(
            None,
            vec![
                config_arg,
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    let (_ob_config_file, ob_config_arg) =
        orderbook::config::Configuration::default().to_cli_args();
    services
        .start_api(vec![
            ob_config_arg,
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Place Order
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 9u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();

    tracing::info!("waiting for first trade");
    onchain.mint_block().await;
    let trade_happened =
        || async { token_b.balanceOf(trader.address()).call().await.unwrap() == order.buy_amount };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check that settlement buffers were traded.
    let settlement_contract_balance = token_b
        .balanceOf(*onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    // Check that internal buffers were used
    assert_eq!(settlement_contract_balance, U256::ZERO);

    // Same order can trade again with external liquidity
    let order = OrderCreation {
        valid_to: model::time::now_in_epoch_seconds() + 301,
        ..order
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();

    tracing::info!("waiting for second trade");
    let trade_happened = || async {
        onchain.mint_block().await;
        token_b.balanceOf(trader.address()).call().await.unwrap()
            == (order.buy_amount * ::alloy::primitives::U256::from(2))
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
}
