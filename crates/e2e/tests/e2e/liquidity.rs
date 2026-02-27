use {
    alloy::{
        primitives::{Address, B256, address},
        providers::{
            Provider,
            ext::{AnvilApi, ImpersonateConfig},
        },
    },
    autopilot::config::Configuration,
    chrono::{NaiveDateTime, Utc},
    contracts::{ERC20, IZeroex},
    e2e::{
        api::zeroex::{Eip712TypedZeroExOrder, ZeroExApi},
        assert_approximately_eq,
        setup::{
            OnchainComponents, Services, TIMEOUT, TestAccount, colocation,
            run_forked_test_with_block_number, wait_for_condition,
        },
    },
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
};

/// The block number from which we will fetch state for the forked tests.
pub const FORK_BLOCK: u64 = 23112197;
pub const USDT_WHALE: Address = address!("F977814e90dA44bFA03b6295A0616a897441aceC");
pub const USDC_WHALE: Address = address!("28c6c06298d514db089934071355e5743bf21d60");

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

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;
    let [trader, zeroex_maker] = onchain.make_accounts(1u64.eth()).await;

    let token_usdc = ERC20::Instance::new(
        address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        web3.provider.clone(),
    );

    let token_usdt = ERC20::Instance::new(
        address!("dac17f958d2ee523a2206206994597c13d831ec7"),
        web3.provider.clone(),
    );

    web3.wallet.register_signer(solver.signer.clone());
    let zeroex = IZeroex::Instance::deployed(&web3.provider).await.unwrap();

    let amount = 500u64.matom();

    // Give trader some USDC
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            token_usdc
                .transfer(trader.address(), amount)
                .from(USDC_WHALE)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Give 0x maker a bit more USDT
    // With a lower amount 0x contract shows much lower fillable amount
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            token_usdt
                .transfer(
                    zeroex_maker.address(),
                    amount * alloy::primitives::U256::from(4),
                )
                .from(USDT_WHALE)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
    // Required for the remaining fillable taker amount
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            token_usdc
                .transfer(solver.address(), amount)
                .from(USDC_WHALE)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    token_usdc
        .approve(onchain.contracts().allowance, amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    // With a lower amount 0x contract shows much lower fillable amount
    token_usdt
        .approve(*zeroex.address(), amount * alloy::primitives::U256::from(4))
        .from(zeroex_maker.address())
        .send_and_watch()
        .await
        .unwrap();
    token_usdc
        .approve(*zeroex.address(), amount)
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let order = OrderCreation {
        sell_token: *token_usdc.address(),
        sell_amount: amount,
        buy_token: *token_usdt.address(),
        buy_amount: amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let chain_id = web3.provider.get_chain_id().await.unwrap();
    let zeroex_liquidity_orders = create_zeroex_liquidity_orders(
        order.clone(),
        zeroex_maker.clone(),
        *zeroex.address(),
        chain_id,
        *onchain.contracts().weth.address(),
    );
    let zeroex_api_port = ZeroExApi::new(zeroex_liquidity_orders.to_vec()).run().await;

    // Place Orders
    let services = Services::new(&onchain).await;
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
        colocation::LiquidityProvider::ZeroEx {
            api_port: zeroex_api_port,
        },
        false,
    );
    let (_config_file, config_arg) =
        Configuration::test("test_solver", solver.address()).to_cli_args();

    services
        .start_autopilot(
            None,
            vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                config_arg,
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

    // Drive solution
    let sell_token_balance_before = token_usdc.balanceOf(trader.address()).call().await.unwrap();
    let buy_token_balance_before = token_usdt.balanceOf(trader.address()).call().await.unwrap();

    services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        token_usdc
            .balanceOf(trader.address())
            .call()
            .await
            .is_ok_and(|balance| balance < sell_token_balance_before)
    })
    .await
    .unwrap();
    wait_for_condition(TIMEOUT, || async {
        token_usdt
            .balanceOf(trader.address())
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
    let expected_filled_amount = amount + amount / alloy::primitives::U256::from(10);
    assert_approximately_eq!(
        alloy::primitives::U256::from(zeroex_order_amounts.filled),
        expected_filled_amount
    );
    assert!(zeroex_order_amounts.fillable > 0u128);
    assert_approximately_eq!(
        alloy::primitives::U256::from(zeroex_order_amounts.fillable),
        (amount * alloy::primitives::U256::from(2)) - expected_filled_amount
    );

    // Fill the remaining part of the 0x order
    let zeroex_order = Eip712TypedZeroExOrder {
        maker_token: *token_usdt.address(),
        taker_token: *token_usdc.address(),
        maker_amount: zeroex_order_amounts.fillable,
        taker_amount: zeroex_order_amounts.fillable,
        // doesn't participate in the hash calculation
        remaining_fillable_taker_amount: 0u128,
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: *zeroex.address(),
        pool: Default::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: alloy::primitives::U256::from(Utc::now().timestamp()),
    }
    .to_order_record(chain_id, *zeroex.address(), zeroex_maker);
    fill_or_kill_zeroex_limit_order(&zeroex, &zeroex_order, solver.address())
        .await
        .unwrap();
    let zeroex_order_amounts = get_zeroex_order_amounts(&zeroex, &zeroex_order)
        .await
        .unwrap();
    assert_approximately_eq!(
        alloy::primitives::U256::from(zeroex_order_amounts.filled),
        (amount * alloy::primitives::U256::from(2)) - expected_filled_amount
    );
    assert_approximately_eq!(
        alloy::primitives::U256::from(zeroex_order_amounts.fillable),
        alloy::primitives::U256::ZERO
    );
}

fn create_zeroex_liquidity_orders(
    order_creation: OrderCreation,
    zeroex_maker: TestAccount,
    zeroex_addr: Address,
    chain_id: u64,
    weth_address: Address,
) -> [shared::zeroex_api::OrderRecord; 3] {
    let typed_order = Eip712TypedZeroExOrder {
        maker_token: order_creation.buy_token,
        taker_token: order_creation.sell_token,
        // fully covers execution costs
        maker_amount: u128::try_from(order_creation.buy_amount).unwrap() * 3,
        taker_amount: u128::try_from(order_creation.sell_amount).unwrap() * 2,
        // makes 0x order partially filled, but the amount is higher than the cowswap order to
        // make sure the 0x order is not overfilled in the end of the e2e test
        remaining_fillable_taker_amount: (order_creation.sell_amount
            * alloy::primitives::U256::from(3)
            / alloy::primitives::U256::from(2))
        .try_into()
        .unwrap(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        // Makes it possible for anyone to fill the order
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: Default::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: alloy::primitives::U256::from(Utc::now().timestamp()),
    };
    let usdt_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.buy_token,
        // the value comes from the `--amount-to-estimate-prices-with` config to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.try_into().unwrap(),
        remaining_fillable_taker_amount: order_creation.sell_amount.try_into().unwrap(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: Default::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: alloy::primitives::U256::from(Utc::now().timestamp()),
    };
    let usdc_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.sell_token,
        // the value comes from the `--amount-to-estimate-prices-with` config to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.try_into().unwrap(),
        remaining_fillable_taker_amount: order_creation.sell_amount.try_into().unwrap(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: Default::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: alloy::primitives::U256::from(Utc::now().timestamp()),
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
    zeroex: &IZeroex::Instance,
    zeroex_order: &shared::zeroex_api::OrderRecord,
) -> anyhow::Result<ZeroExOrderAmounts> {
    Ok(zeroex
        .getLimitOrderRelevantState(
            IZeroex::LibNativeOrder::LimitOrder {
                makerToken: zeroex_order.order().maker_token,
                takerToken: zeroex_order.order().taker_token,
                makerAmount: zeroex_order.order().maker_amount,
                takerAmount: zeroex_order.order().taker_amount,
                takerTokenFeeAmount: zeroex_order.order().taker_token_fee_amount,
                maker: zeroex_order.order().maker,
                taker: zeroex_order.order().taker,
                sender: zeroex_order.order().sender,
                feeRecipient: zeroex_order.order().fee_recipient,
                pool: zeroex_order.order().pool,
                expiry: zeroex_order.order().expiry,
                salt: zeroex_order.order().salt,
            },
            IZeroex::LibSignature::Signature {
                signatureType: zeroex_order.order().signature.signature_type,
                v: zeroex_order.order().signature.v,
                r: zeroex_order.order().signature.r,
                s: zeroex_order.order().signature.s,
            },
        )
        .call()
        .await
        .map(|response| ZeroExOrderAmounts {
            filled: response.orderInfo.takerTokenFilledAmount,
            fillable: response.actualFillableTakerTokenAmount,
        })?)
}

async fn fill_or_kill_zeroex_limit_order(
    zeroex: &IZeroex::Instance,
    zeroex_order: &shared::zeroex_api::OrderRecord,
    from: Address,
) -> anyhow::Result<B256> {
    let order = zeroex_order.order();
    let tx_hash = zeroex
        .fillOrKillLimitOrder(
            IZeroex::LibNativeOrder::LimitOrder {
                makerToken: order.maker_token,
                takerToken: order.taker_token,
                makerAmount: order.maker_amount,
                takerAmount: order.taker_amount,
                takerTokenFeeAmount: order.taker_token_fee_amount,
                maker: order.maker,
                taker: order.taker,
                sender: order.sender,
                feeRecipient: order.fee_recipient,
                pool: order.pool,
                expiry: order.expiry,
                salt: order.salt,
            },
            IZeroex::LibSignature::Signature {
                signatureType: order.signature.signature_type,
                v: order.signature.v,
                r: order.signature.r,
                s: order.signature.s,
            },
            zeroex_order.order().taker_amount,
        )
        .from(from)
        .send()
        .await?
        .watch()
        .await?;

    Ok(tx_hash)
}
