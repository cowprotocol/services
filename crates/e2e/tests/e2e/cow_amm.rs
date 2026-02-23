use {
    alloy::{
        primitives::{Address, Bytes, FixedBytes, U256, address},
        providers::{
            Provider,
            ext::{AnvilApi, ImpersonateConfig},
        },
    },
    autopilot::config::{Configuration, solver::Solver},
    contracts::alloy::{
        ERC20,
        support::{Balances, Signatures},
    },
    driver::domain::eth::NonZeroU256,
    e2e::setup::{
        DeployedContracts,
        OnchainComponents,
        Services,
        TIMEOUT,
        colocation::{self, SolverEngine},
        mock::Mock,
        run_forked_test_with_block_number,
        run_test,
        wait_for_condition,
    },
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderClass, OrderCreation, OrderKind, OrderUid},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
    solvers_dto::solution::{
        BuyTokenBalance,
        Call,
        Kind,
        SellTokenBalance,
        SigningScheme,
        Solution,
    },
    std::collections::{HashMap, HashSet},
};

#[tokio::test]
#[ignore]
async fn local_node_cow_amm_jit() {
    run_test(cow_amm_jit).await;
}

/// Tests that solvers are able to propose and settle cow amm orders
/// on their own in the form of JIT orders.
async fn cow_amm_jit(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(100u64.eth()).await;
    let [bob, cow_amm_owner] = onchain.make_accounts(1000u64.eth()).await;

    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(300_000u64.eth(), 100u64.eth())
        .await;

    // Fund the buffers with a lot of buy tokens so we can pay out the required
    // tokens for 2 orders in the same direction without having to worry about
    // getting the liquidity on-chain.
    dai.mint(
        *onchain.contracts().gp_settlement.address(),
        100_000u64.eth(),
    )
    .await;

    // set up cow_amm
    let oracle = contracts::alloy::cow_amm::CowAmmUniswapV2PriceOracle::Instance::deploy(
        web3.provider.clone(),
    )
    .await
    .unwrap();

    let cow_amm_factory =
        contracts::alloy::cow_amm::CowAmmConstantProductFactory::Instance::deploy(
            web3.provider.clone(),
            *onchain.contracts().gp_settlement.address(),
        )
        .await
        .unwrap();

    // Fund cow amm owner with 2_000 dai and allow factory take them
    dai.mint(cow_amm_owner.address(), 2_000u64.eth()).await;

    dai.approve(*cow_amm_factory.address(), 2_000u64.eth())
        .from(cow_amm_owner.address())
        .send_and_watch()
        .await
        .unwrap();
    // Fund cow amm owner with 1 WETH and allow factory take them
    onchain
        .contracts()
        .weth
        .deposit()
        .value(1u64.eth())
        .from(cow_amm_owner.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(*cow_amm_factory.address(), 1u64.eth())
        .from(cow_amm_owner.address())
        .send_and_watch()
        .await
        .unwrap();

    let pair = onchain
        .contracts()
        .uniswap_v2_factory
        .getPair(*onchain.contracts().weth.address(), *dai.address())
        .call()
        .await
        .expect("failed to get Uniswap V2 pair");

    let cow_amm = cow_amm_factory
        .ammDeterministicAddress(
            cow_amm_owner.address(),
            *dai.address(),
            *onchain.contracts().weth.address(),
        )
        .call()
        .await
        .unwrap();

    // pad with 12 zeros in the front to end up with 32 bytes
    let oracle_data: Vec<_> = std::iter::repeat_n(0u8, 12).chain(pair.to_vec()).collect();
    const APP_DATA: [u8; 32] = [12u8; 32];

    cow_amm_factory
        .create(
            *dai.address(),
            2_000u64.eth(),
            *onchain.contracts().weth.address(),
            1u64.eth(),
            U256::ZERO, // min traded token
            *oracle.address(),
            Bytes::copy_from_slice(&oracle_data),
            FixedBytes(APP_DATA),
        )
        .from(cow_amm_owner.address())
        .send_and_watch()
        .await
        .unwrap();
    let cow_amm = contracts::alloy::cow_amm::CowAmm::Instance::new(cow_amm, web3.provider.clone());

    // Start system with the regular baseline solver as a quoter but a mock solver
    // for the actual solver competition. That way we can handcraft a solution
    // for this test and don't have to implement complete support for CoW AMMs
    // in the baseline solver.
    let mock_solver = Mock::new().await;
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
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
                haircut_bps: 0,
                submission_keys: vec![],
                forwarder_contract: None,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    let services = Services::new(&onchain).await;

    let (_config_file, config_arg) =
        Configuration::test("mock_solver", solver.address()).to_cli_args();

    services
        .start_autopilot(
            None,
            vec![
                config_arg,
                "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Derive the order's valid_to from the blockchain because the cow amm enforces
    // a relatively small valid_to and we initialize the chain with a date in
    // the past so the computer's current time is way ahead of the blockchain.
    let block = web3
        .provider
        .get_block(alloy::eips::BlockId::latest())
        .await
        .unwrap()
        .unwrap();
    let valid_to = u32::try_from(block.header.timestamp).unwrap() + 300;

    // CoW AMM order with a limit price extremely close to the AMM's current price.
    // current price => 1 WETH == 2000 DAI
    // order price => 0.1 WETH == 230 DAI => 1 WETH == 2300 DAI
    // oracle price => 100 WETH == 300000 DAI => 1 WETH == 3000 DAI
    // If this order gets settled around the oracle price it will receive plenty of
    // surplus.
    let cow_amm_order = contracts::alloy::cow_amm::CowAmm::GPv2Order::Data {
        sellToken: *onchain.contracts().weth.address(),
        buyToken: *dai.address(),
        receiver: Default::default(),
        sellAmount: 0.1.eth(),
        buyAmount: 230u64.eth(),
        validTo: valid_to,
        appData: FixedBytes(APP_DATA),
        feeAmount: U256::ZERO,
        kind: FixedBytes::from_slice(
            &const_hex::decode("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775")
                .unwrap(),
        ), // sell order
        partiallyFillable: false,
        sellTokenBalance: FixedBytes::from_slice(
            &const_hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9")
                .unwrap(),
        ), // erc20
        buyTokenBalance: FixedBytes::from_slice(
            &const_hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9")
                .unwrap(),
        ), // erc20
    };
    let trading_params = contracts::alloy::cow_amm::CowAmm::ConstantProduct::TradingParams {
        minTradedToken0: U256::ZERO,
        priceOracle: *oracle.address(),
        priceOracleData: Bytes::copy_from_slice(&oracle_data),
        appData: FixedBytes(APP_DATA),
    };

    // Generate EIP-1271 signature for the CoW AMM order
    let signature = cow_amm::gpv2_order::generate_eip1271_signature(
        &cow_amm_order,
        &trading_params,
        *cow_amm.address(),
    );

    // Generate commit interaction for the pre-interaction
    let cow_amm_commitment_data = cow_amm::gpv2_order::generate_commit_interaction(
        &cow_amm_order,
        &cow_amm,
        &onchain.contracts().domain_separator,
    );
    let cow_amm_commitment = Call {
        target: cow_amm_commitment_data.target,
        value: cow_amm_commitment_data.value,
        calldata: cow_amm_commitment_data.call_data,
    };

    // fund trader "bob" and approve vault relayer
    onchain
        .contracts()
        .weth
        .deposit()
        .from(bob.address())
        .value(0.1.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, U256::MAX)
        .from(bob.address())
        .send_and_watch()
        .await
        .unwrap();

    // place user order with the same limit price as the CoW AMM order
    let user_order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 0.1.eth(), // 0.1 WETH
        buy_token: *dai.address(),
        buy_amount: 230u64.eth(), // 230 DAI
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &bob.signer,
    );
    let user_order_id = services.create_order(&user_order).await.unwrap();

    let amm_balance_before = dai.balanceOf(*cow_amm.address()).call().await.unwrap();
    let bob_balance_before = dai.balanceOf(bob.address()).call().await.unwrap();

    let fee = 0.01.eth(); // 0.01 WETH

    mock_solver.configure_solution(Some(Solution {
        id: 1,
        // assume price of the univ2 pool
        prices: HashMap::from([
            (*dai.address(), 100u64.eth()),
            (*onchain.contracts().weth.address(), 300_000u64.eth()),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                order: solvers_dto::solution::JitOrder {
                    sell_token: cow_amm_order.sellToken,
                    buy_token: cow_amm_order.buyToken,
                    receiver: cow_amm_order.receiver,
                    sell_amount: cow_amm_order.sellAmount,
                    buy_amount: cow_amm_order.buyAmount,
                    partially_fillable: cow_amm_order.partiallyFillable,
                    valid_to: cow_amm_order.validTo,
                    app_data: cow_amm_order.appData.0,
                    kind: Kind::Sell,
                    sell_token_balance: SellTokenBalance::Erc20,
                    buy_token_balance: BuyTokenBalance::Erc20,
                    signing_scheme: SigningScheme::Eip1271,
                    signature,
                },
                executed_amount: cow_amm_order.sellAmount - fee,
                fee: Some(fee),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                order: solvers_dto::solution::OrderUid(user_order_id.0),
                executed_amount: (user_order.sell_amount - fee),
                fee: Some(fee),
            }),
        ],
        pre_interactions: vec![cow_amm_commitment],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
        wrappers: vec![],
    }));

    // Drive solution
    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let amm_balance = dai.balanceOf(*cow_amm.address()).call().await.unwrap();
        let bob_balance = dai.balanceOf(bob.address()).call().await.unwrap();

        let amm_received = amm_balance - amm_balance_before;
        let bob_received = bob_balance - bob_balance_before;

        // bob and CoW AMM both got surplus and an equal amount
        amm_received >= cow_amm_order.buyAmount && bob_received > user_order.buy_amount
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_cow_amm_driver_support() {
    run_forked_test_with_block_number(
        cow_amm_driver_support,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        // block before relevant cow amm was finalized
        20476674,
    )
    .await;
}

/// Tests that the driver is able to generate template orders for indexed
/// cow amms and that they can be settled by solvers like regular orders.
async fn cow_amm_driver_support(web3: Web3) {
    // The Balances SC is deployed many blocks after the cow amm helper contract, so
    // since changing the forked number would result in very costly ~1 year of event
    // syncing, we deploy the following SCs
    let deployed_contracts = {
        let balances = Balances::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();
        let signatures = Signatures::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();
        DeployedContracts {
            balances: Some(*balances.address()),
            signatures: Some(*signatures.address()),
        }
    };
    let mut onchain = OnchainComponents::deployed_with(web3.clone(), deployed_contracts).await;

    let [solver] = onchain.make_solvers_forked(11u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    // find some USDC available onchain
    const USDC_WHALE_MAINNET: Address = address!("28c6c06298d514db089934071355e5743bf21d60");

    // create necessary token instances
    let usdc = ERC20::Instance::new(
        address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        web3.provider.clone(),
    );

    let usdt = ERC20::Instance::new(
        address!("dac17f958d2ee523a2206206994597c13d831ec7"),
        web3.provider.clone(),
    );

    // Unbalance the cow amm enough that baseline is able to rebalance
    // it with the current liquidity.
    const USDC_WETH_COW_AMM: Address = address!("f08d4dea369c456d26a3168ff0024b904f2d8b91");

    let weth_balance = onchain
        .contracts()
        .weth
        .balanceOf(USDC_WETH_COW_AMM)
        .call()
        .await
        .unwrap();
    // Assuming that the pool is balanced, imbalance it by ~30%, so the driver can
    // crate a CoW AMM JIT order. This imbalance shouldn't exceed 50%, since
    // such an order will be rejected by the SC: <https://github.com/balancer/cow-amm/blob/84750b705a02dd600766c5e6a9dd4370386cf0f1/src/contracts/BPool.sol#L250-L252>
    let weth_to_send = weth_balance.checked_div(U256::from(3)).unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(solver.address())
        .value(weth_to_send)
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .transfer(USDC_WETH_COW_AMM, weth_to_send)
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let amm_usdc_balance_before = usdc.balanceOf(USDC_WETH_COW_AMM).call().await.unwrap();

    // Now we create an unfillable order just so the orderbook is not empty.
    // Otherwise all auctions would be skipped because there is no user order to
    // settle.

    // Give trader some USDC
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            usdc.transfer(trader.address(), 1000u64.matom())
                .from(USDC_WHALE_MAINNET)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Approve GPv2 for trading
    usdc.approve(onchain.contracts().allowance, 1000u64.matom())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Empty liquidity of one of the AMMs to test EmptyPoolRemoval maintenance job.
    const ZERO_BALANCE_AMM: Address = address!("b3bf81714f704720dcb0351ff0d42eca61b069fc");
    let pendle_token = ERC20::Instance::new(
        address!("808507121b80c02388fad14726482e061b8da827"),
        web3.provider.clone(),
    );
    let balance = pendle_token
        .balanceOf(ZERO_BALANCE_AMM)
        .call()
        .await
        .unwrap();
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            pendle_token
                .transfer(
                    address!("027e1cbf2c299cba5eb8a2584910d04f1a8aa403"),
                    balance,
                )
                .from(ZERO_BALANCE_AMM)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    assert!(
        pendle_token
            .balanceOf(ZERO_BALANCE_AMM)
            .call()
            .await
            .unwrap()
            .is_zero()
    );

    // spawn a mock solver so we can later assert things about the received auction
    let mock_solver = Mock::new().await;
    colocation::start_driver_with_config_override(
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
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
                haircut_bps: 0,
                submission_keys: vec![],
                forwarder_contract: None,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
        Some(
            r#"
[[contracts.cow-amms]]
helper = "0x3FF0041A614A9E6Bf392cbB961C97DA214E9CB31"
factory = "0xf76c421bAb7df8548604E60deCCcE50477C10462"
"#,
        ),
    );
    let services = Services::new(&onchain).await;

    let (_config_file, config_arg) = Configuration {
        drivers: vec![
            Solver::test("test_solver", solver.address()),
            Solver::test("mock_solver", solver.address()),
        ],
        ..Default::default()
    }
    .to_cli_args();

    services
        .start_autopilot(
            None,
            vec![
                config_arg,
                "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver"
                    .to_string(),
                // it uses an older helper contract that was deployed before the desired cow amm
                "--cow-amm-configs=0xf76c421bAb7df8548604E60deCCcE50477C10462|0x3FF0041A614A9E6Bf392cbB961C97DA214E9CB31|20476672".to_string()
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    onchain.mint_block().await;

    // Place Orders
    let order = OrderCreation {
        sell_token: *usdc.address(),
        sell_amount: 1000u64.matom(),
        buy_token: *usdt.address(),
        buy_amount: 2000u64.matom(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    // Warm up co-located driver by quoting the order (otherwise placing an order
    // may time out)
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: *usdc.address(),
            buy_token: *usdt.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1000u64.matom()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;

    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        // Keep mining blocks to trigger the event indexing logic
        onchain.mint_block().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(1_000)).await;

        let amm_usdc_balance_after = usdc.balanceOf(USDC_WETH_COW_AMM).call().await.unwrap();
        // CoW AMM traded automatically
        amm_usdc_balance_after != amm_usdc_balance_before
    })
    .await
    .unwrap();

    // all cow amms on mainnet the helper contract is aware of
    tracing::info!("Waiting for all cow amms to be indexed.");
    wait_for_condition(TIMEOUT, || async {
        let auctions = mock_solver.get_auctions();
        let found_cow_amms: HashSet<_> = auctions
            .iter()
            .flat_map(|a| a.surplus_capturing_jit_order_owners.clone())
            .collect();

        found_cow_amms.contains(&USDC_WETH_COW_AMM)
    })
    .await
    .unwrap();

    // all tokens traded by the cow amms
    tracing::info!("Waiting for all relevant native prices to be indexed.");
    let expected_prices = [
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), // WETH
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // USDC
    ];

    wait_for_condition(TIMEOUT, || async {
        let auctions = mock_solver.get_auctions();
        let auction_prices: HashSet<_> = auctions
            .iter()
            .flat_map(|auction| {
                auction
                    .tokens
                    .iter()
                    .filter_map(|(token, info)| info.reference_price.map(|_| token))
            })
            .collect();

        let found_amm_jit_orders = auctions.iter().any(|auction| {
            auction.orders.iter().any(|order| {
                order.owner == USDC_WETH_COW_AMM
                    && order.sell_token == *onchain.contracts().weth.address()
                    && order.buy_token == *usdc.address()
            })
        });

        found_amm_jit_orders
            && expected_prices
                .iter()
                .all(|token| auction_prices.contains(token))
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore]
async fn local_node_cow_amm_opposite_direction() {
    run_test(cow_amm_opposite_direction).await;
}

/// Tests that only CoW AMM liquidity can be used to fulfill the order.
async fn cow_amm_opposite_direction(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(100u64.eth()).await;
    let [bob, cow_amm_owner] = onchain.make_accounts(1000u64.eth()).await;

    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(300_000u64.eth(), 100u64.eth())
        .await;

    // No need to fund the buffers since we're testing the CoW AMM directly filling
    // the user order.

    // Set up the CoW AMM as before
    let oracle = contracts::alloy::cow_amm::CowAmmUniswapV2PriceOracle::Instance::deploy(
        web3.provider.clone(),
    )
    .await
    .unwrap();

    let cow_amm_factory =
        contracts::alloy::cow_amm::CowAmmConstantProductFactory::Instance::deploy(
            web3.provider.clone(),
            *onchain.contracts().gp_settlement.address(),
        )
        .await
        .unwrap();

    // Fund the CoW AMM owner with DAI and WETH and approve the factory to transfer
    // them
    dai.mint(cow_amm_owner.address(), 2_000u64.eth()).await;

    dai.approve(*cow_amm_factory.address(), 2_000u64.eth())
        .from(cow_amm_owner.address())
        .send_and_watch()
        .await
        .unwrap();

    onchain
        .contracts()
        .weth
        .deposit()
        .from(cow_amm_owner.address())
        .value(1u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(*cow_amm_factory.address(), 1u64.eth())
        .from(cow_amm_owner.address())
        .send_and_watch()
        .await
        .unwrap();

    onchain
        .contracts()
        .weth
        .deposit()
        .from(solver.address())
        .value(1u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    let pair = onchain
        .contracts()
        .uniswap_v2_factory
        .getPair(*onchain.contracts().weth.address(), *dai.address())
        .call()
        .await
        .expect("failed to get Uniswap V2 pair");

    let cow_amm_address = cow_amm_factory
        .ammDeterministicAddress(
            cow_amm_owner.address(),
            *dai.address(),
            *onchain.contracts().weth.address(),
        )
        .call()
        .await
        .unwrap();

    // pad with 12 zeros to end up with 32 bytes
    let oracle_data: Vec<_> = std::iter::repeat_n(0u8, 12).chain(pair.to_vec()).collect();
    const APP_DATA: [u8; 32] = [12u8; 32];

    // Create the CoW AMM
    cow_amm_factory
        .create(
            *dai.address(),
            2_000u64.eth(),
            *onchain.contracts().weth.address(),
            1u64.eth(),
            U256::ZERO, // min traded token
            *oracle.address(),
            Bytes::copy_from_slice(&oracle_data),
            FixedBytes(APP_DATA),
        )
        .from(cow_amm_owner.address())
        .send_and_watch()
        .await
        .unwrap();
    let cow_amm =
        contracts::alloy::cow_amm::CowAmm::Instance::new(cow_amm_address, web3.provider.clone());

    // Start system with the mocked solver. Baseline is still required for the
    // native price estimation.
    let mock_solver = Mock::new().await;
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
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
                haircut_bps: 0,
                submission_keys: vec![],
                forwarder_contract: None,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        true,
    );
    let services = Services::new(&onchain).await;

    let (_config_file, config_arg) =
        Configuration::test("mock_solver", solver.address()).to_cli_args();

    services
        .start_autopilot(
            None,
            vec![
                config_arg,
                "--price-estimation-drivers=mock_solver|http://localhost:11088/mock_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=mock_solver|http://localhost:11088/mock_solver".to_string(),
        ])
        .await;

    // Get the current block timestamp
    let block = web3
        .provider
        .get_block(alloy::eips::BlockId::latest())
        .await
        .unwrap()
        .unwrap();
    let valid_to = u32::try_from(block.header.timestamp).unwrap() + 300;
    let executed_amount = 230u64.eth();

    // CoW AMM order remains the same (selling WETH for DAI)
    let cow_amm_order = contracts::alloy::cow_amm::CowAmm::GPv2Order::Data {
        sellToken: *onchain.contracts().weth.address(),
        buyToken: *dai.address(),
        receiver: Default::default(),
        sellAmount: 0.1.eth(),
        buyAmount: executed_amount,
        validTo: valid_to,
        appData: FixedBytes(APP_DATA),
        feeAmount: U256::ZERO,
        kind: FixedBytes::from_slice(
            &const_hex::decode("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775")
                .unwrap(),
        ), // sell order
        partiallyFillable: false,
        sellTokenBalance: FixedBytes::from_slice(
            &const_hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9")
                .unwrap(),
        ), // erc20
        buyTokenBalance: FixedBytes::from_slice(
            &const_hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9")
                .unwrap(),
        ), // erc20
    };
    let trading_params = contracts::alloy::cow_amm::CowAmm::ConstantProduct::TradingParams {
        minTradedToken0: U256::ZERO,
        priceOracle: *oracle.address(),
        priceOracleData: Bytes::copy_from_slice(&oracle_data),
        appData: FixedBytes(APP_DATA),
    };
    // Generate EIP-1271 signature for the CoW AMM order
    let signature = cow_amm::gpv2_order::generate_eip1271_signature(
        &cow_amm_order,
        &trading_params,
        *cow_amm.address(),
    );

    // Generate commit interaction for the pre-interaction
    let cow_amm_commitment_data = cow_amm::gpv2_order::generate_commit_interaction(
        &cow_amm_order,
        &cow_amm,
        &onchain.contracts().domain_separator,
    );
    let cow_amm_commitment = Call {
        target: cow_amm_commitment_data.target,
        value: cow_amm_commitment_data.value,
        calldata: cow_amm_commitment_data.call_data,
    };

    // Fund trader "bob" with DAI and approve allowance
    dai.mint(bob.address(), 250u64.eth()).await;

    dai.approve(onchain.contracts().allowance, alloy::primitives::U256::MAX)
        .from(bob.address())
        .send_and_watch()
        .await
        .unwrap();

    // Get balances before the trade
    let amm_weth_balance_before = onchain
        .contracts()
        .weth
        .balanceOf(*cow_amm.address())
        .call()
        .await
        .unwrap();
    let bob_weth_balance_before = onchain
        .contracts()
        .weth
        .balanceOf(bob.address())
        .call()
        .await
        .unwrap();

    // Compensate a delay between the `CurrentBlockStream` and the actual onchain
    // data.
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Set the fees appropriately
    let fee_cow_amm = 0.01.eth(); // 0.01 WETH
    let fee_user = 1u64.eth(); // 1 DAI

    let mocked_solutions = |order_uid: OrderUid| {
        Solution {
            id: 1,
            prices: HashMap::from([
                (*dai.address(), 1u64.eth()),                         // 1 DAI = $1
                (*onchain.contracts().weth.address(), 2300u64.eth()), // 1 WETH = $2300
            ]),
            trades: vec![
                solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                    order: solvers_dto::solution::JitOrder {
                        sell_token: cow_amm_order.sellToken,
                        buy_token: cow_amm_order.buyToken,
                        receiver: cow_amm_order.receiver,
                        sell_amount: cow_amm_order.sellAmount,
                        buy_amount: cow_amm_order.buyAmount,
                        partially_fillable: cow_amm_order.partiallyFillable,
                        valid_to: cow_amm_order.validTo,
                        app_data: cow_amm_order.appData.0,
                        kind: Kind::Sell,
                        sell_token_balance: SellTokenBalance::Erc20,
                        buy_token_balance: BuyTokenBalance::Erc20,
                        signing_scheme: SigningScheme::Eip1271,
                        signature: signature.clone(),
                    },
                    executed_amount: cow_amm_order.sellAmount - fee_cow_amm,
                    fee: Some(fee_cow_amm),
                }),
                solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                    order: solvers_dto::solution::OrderUid(order_uid.0),
                    executed_amount: executed_amount - fee_user,
                    fee: Some(fee_user),
                }),
            ],
            pre_interactions: vec![cow_amm_commitment.clone()],
            interactions: vec![],
            post_interactions: vec![],
            gas: None,
            flashloans: None,
            wrappers: vec![],
        }
    };

    // Configure the mocked `/quote` solver's solution
    let mocked_quote_solution = mocked_solutions(OrderUid([0u8; 56]));
    mock_solver.configure_solution(Some(mocked_quote_solution));

    let quote_request = OrderQuoteRequest {
        from: bob.address(),
        sell_token: *dai.address(),
        buy_token: *onchain.contracts().weth.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::try_from(executed_amount).unwrap(),
            },
        },
        ..Default::default()
    };

    // Must align with the mocked_solutions.
    let quote_response = services.submit_quote(&quote_request).await.unwrap();
    assert!(quote_response.verified);
    assert_eq!(quote_response.quote.sell_token, *dai.address());
    assert_eq!(
        quote_response.quote.buy_token,
        *onchain.contracts().weth.address()
    );
    // Ensure the amounts are the same as the solution proposes.
    assert_eq!(quote_response.quote.sell_amount, executed_amount);
    assert_eq!(quote_response.quote.buy_amount, 0.1.eth());

    // Place user order where bob sells DAI to buy WETH (opposite direction)
    let user_order = OrderCreation {
        sell_token: *dai.address(),
        sell_amount: executed_amount, // 230 DAI
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 0.09.eth(), // 0.09 WETH to generate some surplus
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &bob.signer,
    );
    let user_order_id = services.create_order(&user_order).await.unwrap();

    // Configure the mocked `/solve` solver's solution
    let mocked_solve_solution = mocked_solutions(user_order_id);
    mock_solver.configure_solution(Some(mocked_solve_solution.clone()));

    // Drive solution
    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let amm_weth_balance_after = onchain
            .contracts()
            .weth
            .balanceOf(*cow_amm.address())
            .call()
            .await
            .unwrap();
        let bob_weth_balance_after = onchain
            .contracts()
            .weth
            .balanceOf(bob.address())
            .call()
            .await
            .unwrap();

        let amm_weth_sent = amm_weth_balance_before - amm_weth_balance_after;
        let bob_weth_received = bob_weth_balance_after - bob_weth_balance_before;

        // Bob should receive WETH, CoW AMM's WETH balance decreases
        bob_weth_received >= user_order.buy_amount && amm_weth_sent == cow_amm_order.sellAmount
    })
    .await
    .unwrap();

    // Verify that the trade is indexed
    tracing::info!("Waiting for trade to be indexed.");
    wait_for_condition(TIMEOUT, || async {
        let trades = services.get_trades(&user_order_id).await.ok();
        trades.is_some_and(|trades| !trades.is_empty())
    })
    .await
    .unwrap();
}
