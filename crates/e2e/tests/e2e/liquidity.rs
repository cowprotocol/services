use {
    chrono::{NaiveDateTime, Utc},
    contracts::{i_zero_ex::Contract, IZeroEx, ERC20},
    driver::domain::eth::H160,
    e2e::{
        api::zeroex::{Eip712TypedZeroExOrder, ZeroExApi},
        assert_approximately_eq,
        nodes::forked_node::ForkedNodeApi,
        setup::{
            colocation,
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
    ethcontract::{
        errors::MethodError,
        prelude::U256,
        transaction::TransactionResult,
        Account,
        Bytes,
        H256,
    },
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
        // With a lower amount 0x contract shows much lower fillable amount
        token_usdt.transfer(zeroex_maker.address(), amount * 4)
    );
    // Required for the remaining fillable taker amount
    tx!(usdc_whale, token_usdc.transfer(solver.address(), amount));

    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, amount)
    );
    tx!(
        zeroex_maker.account(),
        // With a lower amount 0x contract shows much lower fillable amount
        token_usdt.approve(zeroex.address(), amount * 4)
    );
    tx!(
        solver.account(),
        token_usdc.approve(zeroex.address(), amount)
    );

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: amount,
        buy_token: token_usdt.address(),
        buy_amount: amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
    let zeroex_liquidity_orders = create_zeroex_liquidity_orders(
        order.clone(),
        zeroex_maker.clone(),
        zeroex.address(),
        chain_id,
        onchain.contracts().weth.address(),
    );
    let zeroex_api_port = ZeroExApi::new(zeroex_liquidity_orders.to_vec()).run().await;

    // Place Orders
    let services = Services::new(onchain.contracts()).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                onchain.contracts().weth.address(),
                vec![],
            )
            .await,
        ],
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

    // We force the block to start before the test, so the auction is not cut by the
    // block in the middle of the operations, creating uncertainty
    onchain.mint_block().await;

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
        onchain.mint_block().await;
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

    let zeroex_order_amounts = get_zeroex_order_amounts(&zeroex, &zeroex_liquidity_orders[0])
        .await
        .unwrap();
    // [`relative-slippage`] config value is set to 0.1
    // crates/e2e/src/setup/colocation.rs:110 which is then applied to the
    // original filled amount crates/solver/src/liquidity/slippage.rs:110
    let expected_filled_amount = amount.as_u128() + amount.as_u128() / 10u128;
    assert_approximately_eq!(
        U256::from(zeroex_order_amounts.filled),
        U256::from(expected_filled_amount)
    );
    assert!(zeroex_order_amounts.fillable > 0u128);
    assert_approximately_eq!(
        U256::from(zeroex_order_amounts.fillable),
        U256::from(amount.as_u128() * 2 - expected_filled_amount)
    );

    // Fill the remaining part of the 0x order
    let zeroex_order = Eip712TypedZeroExOrder {
        maker_token: token_usdt.address(),
        taker_token: token_usdc.address(),
        maker_amount: zeroex_order_amounts.fillable,
        taker_amount: zeroex_order_amounts.fillable,
        // doesn't participate in the hash calculation
        remaining_fillable_taker_amount: 0u128,
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex.address(),
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    }
    .to_order_record(chain_id, zeroex.address(), zeroex_maker);
    fill_or_kill_zeroex_limit_order(&zeroex, &zeroex_order, solver.account().clone())
        .await
        .unwrap();
    let zeroex_order_amounts = get_zeroex_order_amounts(&zeroex, &zeroex_order)
        .await
        .unwrap();
    assert_approximately_eq!(
        U256::from(zeroex_order_amounts.filled),
        U256::from(amount.as_u128() * 2 - expected_filled_amount)
    );
    assert_approximately_eq!(U256::from(zeroex_order_amounts.fillable), U256::zero());
}

fn create_zeroex_liquidity_orders(
    order_creation: OrderCreation,
    zeroex_maker: TestAccount,
    zeroex_addr: H160,
    chain_id: u64,
    weth_address: H160,
) -> [shared::zeroex_api::OrderRecord; 3] {
    let typed_order = Eip712TypedZeroExOrder {
        maker_token: order_creation.buy_token,
        taker_token: order_creation.sell_token,
        // fully covers execution costs
        maker_amount: order_creation.buy_amount.as_u128() * 3,
        taker_amount: order_creation.sell_amount.as_u128() * 2,
        // makes 0x order partially filled, but the amount is higher than the cowswap order to
        // make sure the 0x order is not overfilled in the end of the e2e test
        remaining_fillable_taker_amount: order_creation.sell_amount.as_u128() * 3 / 2,
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        // Makes it possible for anyone to fill the order
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    let usdt_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.buy_token,
        // the value comes from the `--amount-to-estimate-prices-with` config to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.as_u128(),
        remaining_fillable_taker_amount: order_creation.sell_amount.as_u128(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    let usdc_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.sell_token,
        // the value comes from the `--amount-to-estimate-prices-with` config to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.as_u128(),
        remaining_fillable_taker_amount: order_creation.sell_amount.as_u128(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    [typed_order, usdt_weth_order, usdc_weth_order]
        .map(|order| order.to_order_record(chain_id, zeroex_addr, zeroex_maker.clone()))
}

#[derive(Debug)]
struct ZeroExOrderAmounts {
    filled: u128,
    fillable: u128,
}

async fn get_zeroex_order_amounts(
    zeroex: &Contract,
    zeroex_order: &shared::zeroex_api::OrderRecord,
) -> Result<ZeroExOrderAmounts, MethodError> {
    zeroex
        .get_limit_order_relevant_state(
            (
                zeroex_order.order().maker_token,
                zeroex_order.order().taker_token,
                zeroex_order.order().maker_amount,
                zeroex_order.order().taker_amount,
                zeroex_order.order().taker_token_fee_amount,
                zeroex_order.order().maker,
                zeroex_order.order().taker,
                zeroex_order.order().sender,
                zeroex_order.order().fee_recipient,
                Bytes(zeroex_order.order().pool.0),
                zeroex_order.order().expiry,
                zeroex_order.order().salt,
            ),
            (
                zeroex_order.order().signature.signature_type,
                zeroex_order.order().signature.v,
                Bytes(zeroex_order.order().signature.r.0),
                Bytes(zeroex_order.order().signature.s.0),
            ),
        )
        .call()
        .await
        .map(|((_, _, filled), fillable, _)| ZeroExOrderAmounts { filled, fillable })
}

async fn fill_or_kill_zeroex_limit_order(
    zeroex: &Contract,
    zeroex_order: &shared::zeroex_api::OrderRecord,
    from_account: Account,
) -> Result<TransactionResult, MethodError> {
    zeroex
        .fill_or_kill_limit_order(
            (
                zeroex_order.order().maker_token,
                zeroex_order.order().taker_token,
                zeroex_order.order().maker_amount,
                zeroex_order.order().taker_amount,
                zeroex_order.order().taker_token_fee_amount,
                zeroex_order.order().maker,
                zeroex_order.order().taker,
                zeroex_order.order().sender,
                zeroex_order.order().fee_recipient,
                Bytes(zeroex_order.order().pool.0),
                zeroex_order.order().expiry,
                zeroex_order.order().salt,
            ),
            (
                zeroex_order.order().signature.signature_type,
                zeroex_order.order().signature.v,
                Bytes(zeroex_order.order().signature.r.0),
                Bytes(zeroex_order.order().signature.s.0),
            ),
            zeroex_order.order().taker_amount,
        )
        .from(from_account)
        .send()
        .await
}
