use {
    ::alloy::{
        primitives::{Address, U256, address, map::AddressMap},
        providers::Provider,
        rpc::types::state::StateOverride,
    },
    balance_overrides::{BalanceOverrideRequest, StateOverrides, StateOverriding},
    configs::{autopilot::Configuration, test_util::TestDefault},
    contracts::ERC20,
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    number::units::EthUnit,
    serde_json::json,
};

#[tokio::test]
#[ignore]
async fn local_node_standard_verified_quote() {
    run_test(standard_verified_quote).await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_eth_balance() {
    run_test(verified_quote_eth_balance).await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_for_settlement_contract() {
    run_test(verified_quote_for_settlement_contract).await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_with_simulated_balance() {
    run_test(verified_quote_with_simulated_balance).await;
}

#[tokio::test]
#[ignore]
async fn local_node_trace_based_balance_detection() {
    run_test(trace_based_balance_detection).await;
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_usdt_quote() {
    run_forked_test_with_block_number(
        usdt_quote_verification,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        23112197,
    )
    .await;
}

/// Quote verification for an Aave v3 aToken as the sell token.
#[tokio::test]
#[ignore]
async fn forked_node_mainnet_aave_atoken_quote() {
    // Fork a block that matches the solver whitelist used for the quotes we
    // inspected on barn (same day as the debugging session). The default
    // `FORK_BLOCK_MAINNET` is a few weeks older and can miss newly
    // whitelisted solvers.
    const FORK_BLOCK: u64 = 24920000;
    run_forked_test_with_extra_filters_and_block_number(
        aave_atoken_quote_verification,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK,
        ["price_estimation=trace", "balance_overrides=trace"],
    )
    .await;
}

/// Verified quotes work as expected.
async fn standard_verified_quote(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 1u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // quote where the trader has sufficient balance and an approval set.
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *token.address(),
            buy_token: *onchain.contracts().weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Verified quotes work as for WETH trades without wrapping or approvals.
async fn verified_quote_eth_balance(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let weth = &onchain.contracts().weth;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // quote where the trader has no WETH balances or approval set, but
    // sufficient ETH for the trade
    assert!(
        weth.balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    assert!(
        weth.allowance(trader.address(), onchain.contracts().allowance)
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Test that asserts that we can verify quotes where the settlement contract is
/// the trader or receiver.
async fn verified_quote_for_settlement_contract(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(3u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Send 3 ETH to the settlement contract so we can get verified quotes for
    // selling WETH.
    onchain
        .send_wei(*onchain.contracts().gp_settlement.address(), 3u64.eth())
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    let request = OrderQuoteRequest {
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: (3u64.eth()).try_into().unwrap(),
            },
        },
        ..Default::default()
    };

    // quote where settlement contract is trader and implicit receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: *onchain.contracts().gp_settlement.address(),
            receiver: None,
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where settlement contract is trader and explicit receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: *onchain.contracts().gp_settlement.address(),
            receiver: Some(*onchain.contracts().gp_settlement.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where settlement contract is trader and not the receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: *onchain.contracts().gp_settlement.address(),
            receiver: Some(trader.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where a random trader sends funds to the settlement contract
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            receiver: Some(*onchain.contracts().gp_settlement.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Test that asserts that we can verify quotes for traders with simulated
/// balances.
async fn verified_quote_with_simulated_balance(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(0u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let weth = &onchain.contracts().weth;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    let orderbook_config = configs::orderbook::Configuration {
        price_estimation: configs::price_estimation::PriceEstimation {
            balance_overrides: Default::default(),
            ..configs::orderbook::Configuration::test_default().price_estimation
        },
        ..configs::orderbook::Configuration::test_default()
    };
    services
        .start_protocol_with_args(
            Configuration::test("test_solver", solver.address()),
            orderbook_config,
            solver,
        )
        .await;

    // quote where the trader has no balances or approval set from TOKEN->WETH
    assert_eq!(
        (
            token.balanceOf(trader.address()).call().await.unwrap(),
            token
                .allowance(trader.address(), onchain.contracts().allowance)
                .call()
                .await
                .unwrap(),
        ),
        (U256::ZERO, U256::ZERO),
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *token.address(),
            buy_token: *weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where the trader has no balances or approval set from WETH->TOKEN
    assert!(
        onchain
            .web3()
            .provider
            .get_balance(trader.address())
            .await
            .unwrap()
            .is_zero()
    );
    assert!(
        weth.balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    assert!(
        weth.allowance(trader.address(), onchain.contracts().allowance)
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // with balance overrides we can even verify quotes for the 0 address
    // which is used when no wallet is connected in the frontend
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: Address::ZERO,
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // Previously quote verification did not set up the trade correctly
    // if the user provided pre-interactions. This works now.
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: Address::ZERO,
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            app_data: model::order::OrderCreationAppData::Full {
                full: json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "0"
                                }
                            ]
                        }
                    }
                })
                .to_string(),
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Ensures that quotes can even be verified with tokens like `USDT`
/// which are not completely ERC20 compliant.
async fn usdt_quote_verification(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;

    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let usdt = address!("dac17f958d2ee523a2206206994597c13d831ec7");

    // Place Orders
    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            Configuration::test("test_solver", solver.address()),
            configs::orderbook::Configuration {
                price_estimation: configs::price_estimation::PriceEstimation {
                    balance_overrides: Default::default(),
                    ..configs::orderbook::Configuration::test_default().price_estimation
                },
                ..configs::orderbook::Configuration::test_default()
            },
            solver,
        )
        .await;

    let quote = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: usdt,
            buy_token: usdc,
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1000u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(quote.verified);
}

/// Tests that balance override detection works for tokens with non-standard
/// storage layouts, including struct-offset and remote-storage patterns.
async fn trace_based_balance_detection(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let weth = *onchain.contracts().weth.address();

    let struct_offset_token =
        contracts::test::NonStandardERC20Balances::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();

    let local_storage_token =
        contracts::test::RemoteERC20Balances::Instance::deploy(web3.provider.clone(), weth, true)
            .await
            .unwrap();
    let delegated_storage_token =
        contracts::test::RemoteERC20Balances::Instance::deploy(web3.provider.clone(), weth, false)
            .await
            .unwrap();

    struct_offset_token
        .mint(trader.address(), 100u64.eth())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    local_storage_token
        .mint(trader.address(), 123u64.eth())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let test_account = address!("0000000000000000000000000000000000000042");
    let test_balance = U256::from(123_456_789_u64);

    use contracts::ERC20;

    async fn test_balance_override(
        web3: &Web3,
        token: Address,
        test_account: Address,
        test_balance: U256,
    ) {
        let balance_overrides = StateOverrides::new(web3.clone());

        let override_result = balance_overrides
            .balance_override(BalanceOverrideRequest {
                token,
                holder: test_account,
                amount: test_balance,
            })
            .await;

        assert!(override_result.is_some(), "Should produce state override");
        let (override_token, state_override) = override_result.unwrap();

        let token_contract = ERC20::Instance::new(token, web3.provider.clone());
        let balance = token_contract
            .balanceOf(test_account)
            .state(AddressMap::from_iter([(
                override_token,
                state_override.clone(),
            )]))
            .call()
            .await
            .unwrap();

        assert_eq!(
            balance, test_balance,
            "Balance override should work for token {:?}",
            token
        );

        tracing::info!(
            ?token,
            ?balance,
            ?override_token,
            ?state_override,
            "✓ Balance override verified for token",
        );
    }

    test_balance_override(&web3, weth, test_account, test_balance).await;
    test_balance_override(
        &web3,
        *struct_offset_token.address(),
        test_account,
        test_balance,
    )
    .await;
    test_balance_override(
        &web3,
        *delegated_storage_token.address(),
        test_account,
        test_balance,
    )
    .await;
    test_balance_override(
        &web3,
        *local_storage_token.address(),
        test_account,
        test_balance,
    )
    .await;
}

/// Exercises the `AaveV3AToken` balance override strategy against a real
/// mainnet fork. We build the override with the forked web3 (so it reads
/// the live `getReserveNormalizedIncome` from the Aave v3 Pool), then apply
/// it in an `eth_call` to `aToken.balanceOf(holder)` and assert the
/// reported balance matches the requested amount within one wei of ray
/// rounding. This is the property `TradeVerifier` relies on when using
/// the override to fund the spardose.
async fn aave_atoken_quote_verification(web3: Web3) {
    // aEthWETH / WETH / Aave v3 Pool on mainnet.
    let a_eth_weth = address!("4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8");
    let spardose = address!("0000000000000000000000000000000000020000");

    let balance_overrides = StateOverrides::new(web3.clone());

    let amount = 5u64.eth(); // 5 aEthWETH

    let (target, override_) = balance_overrides
        .balance_override(BalanceOverrideRequest {
            token: a_eth_weth,
            holder: spardose,
            amount,
        })
        .await
        .expect("override computed");
    assert_eq!(target, a_eth_weth);

    // Apply the override to a live `balanceOf` call via the forked node and
    // make sure the contract now reports the requested amount (± 1 wei of
    // ray rounding).
    let overrides: StateOverride = [(target, override_)].into_iter().collect();
    let a_token_contract = ERC20::Instance::new(a_eth_weth, web3.provider.clone());
    let reported = a_token_contract
        .balanceOf(spardose)
        .state(overrides)
        .call()
        .await
        .unwrap();

    let diff = reported.abs_diff(amount);
    assert!(
        diff <= U256::from(1u64),
        "balanceOf after override returned {reported}, expected ~{amount} (diff {diff} wei)",
    );
}
