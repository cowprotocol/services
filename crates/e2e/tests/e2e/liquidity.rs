use {
    chrono::{NaiveDateTime, Utc},
    contracts::{IZeroEx, ERC20},
    driver::domain::eth::H160,
    e2e::{
        api::zeroex::{Eip712TypedZeroExOrder, ZeroExApi},
        nodes::forked_node::ForkedNodeApi,
        setup::{
            colocation::{self, SolverEngine},
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
            OnchainComponents,
            Services,
            TestAccount,
            TIMEOUT,
        },
        tx,
    },
    ethcontract::{prelude::U256, H256},
    ethrpc::Web3,
    hex_literal::hex,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    web3::signing::SecretKeyRef,
};

/// The block number from which we will fetch state for the forked tests.
pub const FORK_BLOCK: u64 = 18477910;
pub const USDT_WHALE: H160 = H160(hex!("F977814e90dA44bFA03b6295A0616a897441aceC"));

#[tokio::test]
#[ignore]
async fn forked_node_zero_ex_liquidity_mainnet() {
    run_forked_test_with_block_number(
        zero_ex_liquidity,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK,
    )
    .await
}

async fn zero_ex_liquidity(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    let [trader, zeroex_maker] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_usdt = ERC20::at(
        &web3,
        "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .unwrap(),
    );

    let zeroex = IZeroEx::deployed(&web3).await.unwrap();

    let amount = to_wei_with_exp(5, 8);

    // Give trader some USDC
    let usdc_whale = forked_node_api.impersonate(&USDT_WHALE).await.unwrap();
    tx!(usdc_whale, token_usdc.transfer(trader.address(), amount));

    // Give 0x maker a bit more USDT
    let usdt_whale = forked_node_api.impersonate(&USDT_WHALE).await.unwrap();
    tx!(
        usdt_whale,
        token_usdt.transfer(zeroex_maker.address(), amount * 2)
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, amount)
    );
    tx!(
        zeroex_maker.account(),
        token_usdt.approve(zeroex.address(), amount * 2)
    );

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: amount,
        buy_token: token_usdt.address(),
        buy_amount: amount - to_wei_with_exp(1, 8),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let zeroex_api_port = {
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let zeroex_liquidity_orders = create_zeroex_liquidity_orders(
            order.clone(),
            zeroex_maker,
            zeroex.address(),
            onchain.contracts().gp_settlement.address(),
            chain_id,
            onchain.contracts().weth.address(),
        );

        ZeroExApi::new(zeroex_liquidity_orders).run().await
    };

    // Place Orders
    let services = Services::new(onchain.contracts()).await;
    let solver_endpoint =
        colocation::start_baseline_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver,
            endpoint: solver_endpoint,
        }],
        colocation::LiquidityProvider::ZeroEx {
            api_port: zeroex_api_port,
        },
    );
    services
        .start_autopilot(
            None,
            vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Drive solution
    let sell_token_balance_before = token_usdc
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    let buy_token_balance_before = token_usdt
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        token_usdc
            .balance_of(trader.address())
            .call()
            .await
            .is_ok_and(|balance| balance < sell_token_balance_before)
    })
    .await
    .unwrap();
    wait_for_condition(TIMEOUT, || async {
        token_usdt
            .balance_of(trader.address())
            .call()
            .await
            .is_ok_and(|balance| balance >= buy_token_balance_before + amount)
    })
    .await
    .unwrap();
}

fn create_zeroex_liquidity_orders(
    order_creation: OrderCreation,
    zeroex_maker: TestAccount,
    zeroex_addr: H160,
    gpv2_addr: H160,
    chain_id: u64,
    weth_address: H160,
) -> Vec<shared::zeroex_api::OrderRecord> {
    let typed_order = Eip712TypedZeroExOrder {
        maker_token: order_creation.buy_token,
        taker_token: order_creation.sell_token,
        // fully covers execution costs
        maker_amount: order_creation.buy_amount.as_u128() * 3,
        taker_amount: order_creation.sell_amount.as_u128() * 2,
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: gpv2_addr,
        sender: gpv2_addr,
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    let usdt_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.buy_token,
        // the value comes from the `--amount-to-estimate-prices-with` config value to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.as_u128(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: gpv2_addr,
        sender: gpv2_addr,
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    let usdc_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.sell_token,
        // the value comes from the `--amount-to-estimate-prices-with` config value to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.as_u128(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: gpv2_addr,
        sender: gpv2_addr,
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    [typed_order, usdt_weth_order, usdc_weth_order]
        .map(|order| order.to_order_record(chain_id, zeroex_addr, zeroex_maker.clone()))
        .to_vec()
}
