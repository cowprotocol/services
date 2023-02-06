use {
    crate::{
        onchain_components::{
            deploy_token_with_weth_uniswap_pool,
            to_wei,
            uniswap_pair_provider,
            WethPoolConfig,
        },
        services::{
            create_order_converter,
            create_orderbook_api,
            wait_for_solvable_orders,
            OrderbookServices,
            API_HOST,
        },
    },
    contracts::IUniswapLikeRouter,
    ethcontract::prelude::{Account, Address, PrivateKey, U256},
    model::{
        order::{OrderBuilder, OrderKind, SellTokenSource},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::{
        code_fetching::MockCodeFetching,
        ethrpc::Web3,
        http_client::HttpClientFactory,
        sources::uniswap_v2::pool_fetching::PoolFetcher,
    },
    solver::{
        liquidity::uniswap_v2::UniswapLikeLiquidity,
        liquidity_collector::LiquidityCollector,
        metrics::NoopMetrics,
        settlement_access_list::{create_priority_estimator, AccessListEstimatorType},
        settlement_submission::{
            submitter::{
                public_mempool_api::{PublicMempoolApi, SubmissionNode, SubmissionNodeKind},
                Strategy,
            },
            GlobalTxPool,
            SolutionSubmitter,
            StrategyArgs,
        },
    },
    std::{sync::Arc, time::Duration},
    web3::signing::SecretKeyRef,
};

const TRADER: [u8; 32] = [1; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_vault_balances() {
    crate::local_node::test(vault_balances).await;
}

async fn vault_balances(web3: Web3) {
    shared::tracing::initialize_reentrant("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader = Account::Offline(PrivateKey::from_raw(TRADER).unwrap(), None);

    // Create & mint tokens to trade, pools for fee connections
    let token = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(100_000),
            weth_amount: to_wei(100_000),
        },
    )
    .await;
    token.mint(trader.address(), to_wei(10)).await;
    let token = token.contract;

    // Approve GPv2 for trading
    tx!(
        trader,
        token.approve(contracts.balancer_vault.address(), to_wei(10))
    );
    tx!(
        trader,
        contracts
            .balancer_vault
            .set_relayer_approval(trader.address(), contracts.allowance, true)
    );

    let OrderbookServices {
        block_stream,
        solvable_orders_cache: _solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, false).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    // Place Orders
    let order = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(9))
        .with_sell_token_balance(SellTokenSource::External)
        .with_fee_amount(to_wei(1))
        .with_buy_token(contracts.weth.address())
        .with_buy_amount(to_wei(8))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    wait_for_solvable_orders(&client, 1).await.unwrap();

    // Drive solution
    let uniswap_pair_provider = uniswap_pair_provider(&contracts);
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, contracts.uniswap_router.address()),
        contracts.gp_settlement.clone(),
        web3.clone(),
        Arc::new(PoolFetcher::uniswap(uniswap_pair_provider, web3.clone())),
    );
    let solver = solver::solver::naive_solver(solver_account);
    let liquidity_collector = LiquidityCollector {
        liquidity_sources: vec![Box::new(uniswap_liquidity)],
        base_tokens,
    };
    let network_id = web3.net().version().await.unwrap();
    let submitted_transactions = GlobalTxPool::default();
    let mut driver = solver::driver::Driver::new(
        contracts.gp_settlement.clone(),
        liquidity_collector,
        vec![solver],
        Arc::new(web3.clone()),
        Duration::from_secs(30),
        contracts.weth.address(),
        Duration::from_secs(0),
        Arc::new(NoopMetrics::default()),
        web3.clone(),
        network_id.clone(),
        Duration::from_secs(30),
        block_stream,
        SolutionSubmitter {
            web3: web3.clone(),
            contract: contracts.gp_settlement.clone(),
            gas_price_estimator: Arc::new(web3.clone()),
            target_confirm_time: Duration::from_secs(1),
            gas_price_cap: f64::MAX,
            max_confirm_time: Duration::from_secs(120),
            retry_interval: Duration::from_secs(5),
            transaction_strategies: vec![
                solver::settlement_submission::TransactionStrategy::PublicMempool(StrategyArgs {
                    submit_api: Box::new(PublicMempoolApi::new(
                        vec![SubmissionNode::new(
                            SubmissionNodeKind::Broadcast,
                            web3.clone(),
                        )],
                        false,
                    )),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::PublicMempool),
                }),
            ],
            access_list_estimator: Arc::new(
                create_priority_estimator(
                    &web3,
                    &[AccessListEstimatorType::Web3],
                    None,
                    network_id,
                )
                .unwrap(),
            ),
            code_fetcher: Arc::new(MockCodeFetching::new()),
        },
        create_orderbook_api(),
        create_order_converter(&web3, contracts.weth.address()),
        15000000u128,
        1.0,
        None,
        None.into(),
        None,
        0,
        Arc::new(MockCodeFetching::new()),
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch token balance");
    assert_eq!(balance, U256::zero());

    let balance = contracts
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .expect("Couldn't fetch native token balance");
    assert_eq!(balance, U256::from(8_972_194_924_949_384_291_u128));
}
