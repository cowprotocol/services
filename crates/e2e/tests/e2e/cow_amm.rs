use {
    app_data::AppDataHash,
    autopilot::util::conv::U256Ext,
    contracts::{
        ERC20,
        alloy::support::{Balances, Signatures},
    },
    driver::domain::eth::NonZeroU256,
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{
            DeployedContracts,
            OnchainComponents,
            Services,
            TIMEOUT,
            colocation::{self, SolverEngine},
            eth,
            mock::Mock,
            run_forked_test_with_block_number,
            run_test,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
        },
        tx,
        tx_value,
    },
    ethcontract::{BlockId, BlockNumber, H160, U256, web3::ethabi::Token},
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
    model::{
        order::{OrderClass, OrderCreation, OrderData, OrderKind, OrderUid},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::{EcdsaSigningScheme, hashed_eip712_message},
    },
    secp256k1::SecretKey,
    shared::{addr, ethrpc::Web3},
    solvers_dto::solution::{
        BuyTokenBalance,
        Call,
        Kind,
        SellTokenBalance,
        SigningScheme,
        Solution,
    },
    std::collections::{HashMap, HashSet},
    web3::signing::SecretKeyRef,
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

    let [solver] = onchain.make_solvers(to_wei(100)).await;
    let [bob, cow_amm_owner] = onchain.make_accounts(to_wei(1000)).await;

    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(300_000), to_wei(100))
        .await;

    // Fund the buffers with a lot of buy tokens so we can pay out the required
    // tokens for 2 orders in the same direction without having to worry about
    // getting the liquidity on-chain.
    dai.mint(onchain.contracts().gp_settlement.address(), to_wei(100_000))
        .await;

    // set up cow_amm
    let oracle = contracts::CowAmmUniswapV2PriceOracle::builder(&web3)
        .deploy()
        .await
        .unwrap();

    let cow_amm_factory = contracts::CowAmmConstantProductFactory::builder(
        &web3,
        onchain.contracts().gp_settlement.address(),
    )
    .deploy()
    .await
    .unwrap();

    // Fund cow amm owner with 2_000 dai and allow factory take them
    dai.mint(cow_amm_owner.address(), to_wei(2_000)).await;

    dai.approve(cow_amm_factory.address().into_alloy(), eth(2_000))
        .from(cow_amm_owner.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
    // Fund cow amm owner with 1 WETH and allow factory take them
    tx_value!(
        cow_amm_owner.account(),
        to_wei(1),
        onchain.contracts().weth.deposit()
    );
    tx!(
        cow_amm_owner.account(),
        onchain
            .contracts()
            .weth
            .approve(cow_amm_factory.address(), to_wei(1))
    );

    let pair = onchain
        .contracts()
        .uniswap_v2_factory
        .getPair(
            onchain.contracts().weth.address().into_alloy(),
            *dai.address(),
        )
        .call()
        .await
        .expect("failed to get Uniswap V2 pair");

    let cow_amm = cow_amm_factory
        .amm_deterministic_address(
            cow_amm_owner.address(),
            dai.address().into_legacy(),
            onchain.contracts().weth.address(),
        )
        .call()
        .await
        .unwrap();

    // pad with 12 zeros in the front to end up with 32 bytes
    let oracle_data: Vec<_> = std::iter::repeat_n(0u8, 12).chain(pair.to_vec()).collect();
    const APP_DATA: [u8; 32] = [12u8; 32];

    cow_amm_factory
        .create(
            dai.address().into_legacy(),
            to_wei(2_000),
            onchain.contracts().weth.address(),
            to_wei(1),
            0.into(), // min traded token
            oracle.address(),
            ethcontract::Bytes(oracle_data.clone()),
            ethcontract::Bytes(APP_DATA),
        )
        .from(cow_amm_owner.account().clone())
        .send()
        .await
        .unwrap();
    let cow_amm = contracts::CowAmm::at(&web3, cow_amm);

    // Start system with the regular baseline solver as a quoter but a mock solver
    // for the actual solver competition. That way we can handcraft a solution
    // for this test and don't have to implement complete support for CoW AMMs
    // in the baseline solver.
    let mock_solver = Mock::default();
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
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
            },
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
                    "--drivers=mock_solver|http://localhost:11088/mock_solver|{}",
                    const_hex::encode(solver.address())
                ),
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
        .eth()
        .block(BlockId::Number(BlockNumber::Latest))
        .await
        .unwrap()
        .unwrap();
    let valid_to = block.timestamp.as_u32() + 300;

    // CoW AMM order with a limit price extremely close to the AMM's current price.
    // current price => 1 WETH == 2000 DAI
    // order price => 0.1 WETH == 230 DAI => 1 WETH == 2300 DAI
    // oracle price => 100 WETH == 300000 DAI => 1 WETH == 3000 DAI
    // If this order gets settled around the oracle price it will receive plenty of
    // surplus.
    let cow_amm_order = OrderData {
        sell_token: onchain.contracts().weth.address(),
        buy_token: dai.address().into_legacy(),
        receiver: None,
        sell_amount: U256::exp10(17),
        buy_amount: to_wei(230),
        valid_to,
        app_data: AppDataHash(APP_DATA),
        fee_amount: 0.into(),
        kind: OrderKind::Sell,
        partially_fillable: false,
        sell_token_balance: Default::default(),
        buy_token_balance: Default::default(),
    };

    // structure of signature copied from
    // <https://github.com/cowprotocol/cow-amm/blob/main/test/e2e/ConstantProduct.t.sol#L179>
    let signature_data = ethcontract::web3::ethabi::encode(&[
        Token::Tuple(vec![
            Token::Address(cow_amm_order.sell_token),
            Token::Address(cow_amm_order.buy_token),
            Token::Address(cow_amm_order.receiver.unwrap_or_default()),
            Token::Uint(cow_amm_order.sell_amount),
            Token::Uint(cow_amm_order.buy_amount),
            Token::Uint(cow_amm_order.valid_to.into()),
            Token::FixedBytes(cow_amm_order.app_data.0.to_vec()),
            Token::Uint(cow_amm_order.fee_amount),
            // enum hashes taken from
            // <https://github.com/cowprotocol/contracts/blob/main/src/contracts/libraries/GPv2Order.sol#L50-L79>
            Token::FixedBytes(
                const_hex::decode(
                    "f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775",
                )
                .unwrap(),
            ), // sell order
            Token::Bool(cow_amm_order.partially_fillable),
            Token::FixedBytes(
                const_hex::decode(
                    "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9",
                )
                .unwrap(),
            ), // sell_token_source == erc20
            Token::FixedBytes(
                const_hex::decode(
                    "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9",
                )
                .unwrap(),
            ), // buy_token_destination == erc20
        ]),
        Token::Tuple(vec![
            Token::Uint(0.into()), // min_traded_token
            Token::Address(oracle.address()),
            Token::Bytes(oracle_data),
            Token::FixedBytes(APP_DATA.to_vec()),
        ]),
    ]);

    // Prepend CoW AMM address to the signature so settlement contract know which
    // contract this signature refers to.
    let signature = cow_amm
        .address()
        .as_bytes()
        .iter()
        .cloned()
        .chain(signature_data)
        .collect();

    // Creation of commitment copied from
    // <https://github.com/cowprotocol/cow-amm/blob/main/test/e2e/ConstantProduct.t.sol#L181-L188>
    let cow_amm_commitment = {
        let order_hash = cow_amm_order.hash_struct();
        let order_hash = hashed_eip712_message(&onchain.contracts().domain_separator, &order_hash);
        let commitment = cow_amm
            .commit(ethcontract::Bytes(order_hash))
            .tx
            .data
            .unwrap();
        Call {
            target: cow_amm.address(),
            value: 0.into(),
            calldata: commitment.0.to_vec(),
        }
    };

    // fund trader "bob" and approve vault relayer
    tx_value!(
        bob.account(),
        U256::exp10(17),
        onchain.contracts().weth.deposit()
    );
    tx!(
        bob.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, U256::MAX)
    );

    // place user order with the same limit price as the CoW AMM order
    let user_order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: U256::exp10(17), // 0.1 WETH
        buy_token: dai.address().into_legacy(),
        buy_amount: to_wei(230), // 230 DAI
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(bob.private_key()).unwrap()),
    );
    let user_order_id = services.create_order(&user_order).await.unwrap();

    let amm_balance_before = dai
        .balanceOf(cow_amm.address().into_alloy())
        .call()
        .await
        .unwrap();
    let bob_balance_before = dai
        .balanceOf(bob.address().into_alloy())
        .call()
        .await
        .unwrap();

    let fee = U256::exp10(16); // 0.01 WETH

    mock_solver.configure_solution(Some(Solution {
        id: 1,
        // assume price of the univ2 pool
        prices: HashMap::from([
            (dai.address().into_legacy(), to_wei(100)),
            (onchain.contracts().weth.address(), to_wei(300_000)),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                order: solvers_dto::solution::JitOrder {
                    sell_token: cow_amm_order.sell_token,
                    buy_token: cow_amm_order.buy_token,
                    receiver: cow_amm_order.receiver.unwrap_or_default(),
                    sell_amount: cow_amm_order.sell_amount,
                    buy_amount: cow_amm_order.buy_amount,
                    partially_fillable: cow_amm_order.partially_fillable,
                    valid_to: cow_amm_order.valid_to,
                    app_data: cow_amm_order.app_data.0,
                    kind: Kind::Sell,
                    sell_token_balance: SellTokenBalance::Erc20,
                    buy_token_balance: BuyTokenBalance::Erc20,
                    signing_scheme: SigningScheme::Eip1271,
                    signature,
                },
                executed_amount: cow_amm_order.sell_amount - fee,
                fee: Some(fee),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                order: solvers_dto::solution::OrderUid(user_order_id.0),
                executed_amount: user_order.sell_amount - fee,
                fee: Some(fee),
            }),
        ],
        pre_interactions: vec![cow_amm_commitment],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
    }));

    // Drive solution
    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let amm_balance = dai
            .balanceOf(cow_amm.address().into_alloy())
            .call()
            .await
            .unwrap();
        let bob_balance = dai
            .balanceOf(bob.address().into_alloy())
            .call()
            .await
            .unwrap();

        let amm_received = amm_balance - amm_balance_before;
        let bob_received = bob_balance - bob_balance_before;

        // bob and CoW AMM both got surplus and an equal amount
        amm_received >= cow_amm_order.buy_amount.into_alloy()
            && bob_received > user_order.buy_amount.into_alloy()
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
        let balances = Balances::Instance::deploy(web3.alloy.clone())
            .await
            .unwrap();
        let signatures = Signatures::Instance::deploy(web3.alloy.clone())
            .await
            .unwrap();
        DeployedContracts {
            balances: Some(balances.address().into_legacy()),
            signatures: Some(signatures.address().into_legacy()),
        }
    };
    let mut onchain = OnchainComponents::deployed_with(web3.clone(), deployed_contracts).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(11)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;

    // find some USDC available onchain
    const USDC_WHALE_MAINNET: H160 = H160(hex_literal::hex!(
        "28c6c06298d514db089934071355e5743bf21d60"
    ));
    let usdc_whale = forked_node_api
        .impersonate(&USDC_WHALE_MAINNET)
        .await
        .unwrap();

    // create necessary token instances
    let usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let usdt = ERC20::at(
        &web3,
        "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .unwrap(),
    );

    // Unbalance the cow amm enough that baseline is able to rebalance
    // it with the current liquidity.
    const USDC_WETH_COW_AMM: H160 = H160(hex_literal::hex!(
        "f08d4dea369c456d26a3168ff0024b904f2d8b91"
    ));

    let weth_balance = onchain
        .contracts()
        .weth
        .balance_of(USDC_WETH_COW_AMM)
        .call()
        .await
        .unwrap();
    // Assuming that the pool is balanced, imbalance it by 30%, so the driver can
    // crate a CoW AMM JIT order. This imbalance shouldn't exceed 50%, since
    // such an order will be rejected by the SC: <https://github.com/balancer/cow-amm/blob/84750b705a02dd600766c5e6a9dd4370386cf0f1/src/contracts/BPool.sol#L250-L252>
    let weth_to_send = weth_balance.checked_mul_f64(0.3).unwrap();
    tx_value!(
        solver.account(),
        weth_to_send,
        onchain.contracts().weth.deposit()
    );
    tx!(
        solver.account(),
        onchain
            .contracts()
            .weth
            .transfer(USDC_WETH_COW_AMM, weth_to_send)
    );

    let amm_usdc_balance_before = usdc.balance_of(USDC_WETH_COW_AMM).call().await.unwrap();

    // Now we create an unfillable order just so the orderbook is not empty.
    // Otherwise all auctions would be skipped because there is no user order to
    // settle.

    // Give trader some USDC
    tx!(
        usdc_whale,
        usdc.transfer(trader.address(), to_wei_with_exp(1000, 6))
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        usdc.approve(onchain.contracts().allowance, to_wei_with_exp(1000, 6))
    );

    // Empty liquidity of one of the AMMs to test EmptyPoolRemoval maintenance job.
    let zero_balance_amm = addr!("b3bf81714f704720dcb0351ff0d42eca61b069fc");
    let zero_balance_amm_account = forked_node_api
        .impersonate(&zero_balance_amm)
        .await
        .unwrap();
    let pendle_token = ERC20::at(&web3, addr!("808507121b80c02388fad14726482e061b8da827"));
    let balance = pendle_token
        .balance_of(zero_balance_amm)
        .call()
        .await
        .unwrap();
    tx!(
        zero_balance_amm_account,
        pendle_token.transfer(addr!("027e1cbf2c299cba5eb8a2584910d04f1a8aa403"), balance)
    );
    assert!(
        pendle_token
            .balance_of(zero_balance_amm)
            .call()
            .await
            .unwrap()
            .is_zero()
    );

    // spawn a mock solver so we can later assert things about the received auction
    let mock_solver = Mock::default();
    colocation::start_driver_with_config_override(
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
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
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

    services
        .start_autopilot(
            None,
            vec![
                format!("--drivers=test_solver|http://localhost:11088/test_solver|{},mock_solver|http://localhost:11088/mock_solver|{}", const_hex::encode(solver.address()), const_hex::encode(solver.address())),
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
        sell_token: usdc.address(),
        sell_amount: to_wei_with_exp(1000, 6),
        buy_token: usdt.address(),
        buy_amount: to_wei_with_exp(2000, 6),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    // Warm up co-located driver by quoting the order (otherwise placing an order
    // may time out)
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: usdc.address(),
            buy_token: usdt.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(1000, 6).try_into().unwrap(),
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

        let amm_usdc_balance_after = usdc.balance_of(USDC_WETH_COW_AMM).call().await.unwrap();
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
        addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), // WETH
        addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // USDC
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
                    && order.sell_token == H160(onchain.contracts().weth.address().0)
                    && order.buy_token == usdc.address()
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

    let [solver] = onchain.make_solvers(to_wei(100)).await;
    let [bob, cow_amm_owner] = onchain.make_accounts(to_wei(1000)).await;

    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(300_000), to_wei(100))
        .await;

    // No need to fund the buffers since we're testing the CoW AMM directly filling
    // the user order.

    // Set up the CoW AMM as before
    let oracle = contracts::CowAmmUniswapV2PriceOracle::builder(&web3)
        .deploy()
        .await
        .unwrap();

    let cow_amm_factory = contracts::CowAmmConstantProductFactory::builder(
        &web3,
        onchain.contracts().gp_settlement.address(),
    )
    .deploy()
    .await
    .unwrap();

    // Fund the CoW AMM owner with DAI and WETH and approve the factory to transfer
    // them
    dai.mint(cow_amm_owner.address(), to_wei(2_000)).await;

    dai.approve(cow_amm_factory.address().into_alloy(), eth(2_000))
        .from(cow_amm_owner.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    tx_value!(
        cow_amm_owner.account(),
        to_wei(1),
        onchain.contracts().weth.deposit()
    );
    tx!(
        cow_amm_owner.account(),
        onchain
            .contracts()
            .weth
            .approve(cow_amm_factory.address(), to_wei(1))
    );

    tx_value!(
        solver.account(),
        to_wei(1),
        onchain.contracts().weth.deposit()
    );

    let pair = onchain
        .contracts()
        .uniswap_v2_factory
        .getPair(
            onchain.contracts().weth.address().into_alloy(),
            *dai.address(),
        )
        .call()
        .await
        .expect("failed to get Uniswap V2 pair");

    let cow_amm_address = cow_amm_factory
        .amm_deterministic_address(
            cow_amm_owner.address(),
            dai.address().into_legacy(),
            onchain.contracts().weth.address(),
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
            dai.address().into_legacy(),
            to_wei(2_000),
            onchain.contracts().weth.address(),
            to_wei(1),
            0.into(), // min traded token
            oracle.address(),
            ethcontract::Bytes(oracle_data.clone()),
            ethcontract::Bytes(APP_DATA),
        )
        .from(cow_amm_owner.account().clone())
        .send()
        .await
        .unwrap();
    let cow_amm = contracts::CowAmm::at(&web3, cow_amm_address);

    // Start system with the mocked solver. Baseline is still required for the
    // native price estimation.
    let mock_solver = Mock::default();
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
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        true,
    );
    let services = Services::new(&onchain).await;
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=mock_solver|http://localhost:11088/mock_solver|{}",
                    const_hex::encode(solver.address())
                ),
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
        .eth()
        .block(BlockId::Number(BlockNumber::Latest))
        .await
        .unwrap()
        .unwrap();
    let valid_to = block.timestamp.as_u32() + 300;
    let executed_amount = to_wei(230);

    // CoW AMM order remains the same (selling WETH for DAI)
    let cow_amm_order = OrderData {
        sell_token: onchain.contracts().weth.address(),
        buy_token: dai.address().into_legacy(),
        receiver: None,
        sell_amount: U256::exp10(17), // 0.1 WETH
        buy_amount: executed_amount,  // 230 DAI
        valid_to,
        app_data: AppDataHash(APP_DATA),
        fee_amount: 0.into(),
        kind: OrderKind::Sell,
        partially_fillable: false,
        sell_token_balance: Default::default(),
        buy_token_balance: Default::default(),
    };

    // Create the signature for the CoW AMM order
    let signature_data = ethcontract::web3::ethabi::encode(&[
        Token::Tuple(vec![
            Token::Address(cow_amm_order.sell_token),
            Token::Address(cow_amm_order.buy_token),
            Token::Address(cow_amm_order.receiver.unwrap_or_default()),
            Token::Uint(cow_amm_order.sell_amount),
            Token::Uint(cow_amm_order.buy_amount),
            Token::Uint(cow_amm_order.valid_to.into()),
            Token::FixedBytes(cow_amm_order.app_data.0.to_vec()),
            Token::Uint(cow_amm_order.fee_amount),
            Token::FixedBytes(
                const_hex::decode(
                    "f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775",
                )
                .unwrap(),
            ), // sell order
            Token::Bool(cow_amm_order.partially_fillable),
            Token::FixedBytes(
                const_hex::decode(
                    "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9",
                )
                .unwrap(),
            ), // sell_token_source == erc20
            Token::FixedBytes(
                const_hex::decode(
                    "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9",
                )
                .unwrap(),
            ), // buy_token_destination == erc20
        ]),
        Token::Tuple(vec![
            Token::Uint(0.into()), // min_traded_token
            Token::Address(oracle.address()),
            Token::Bytes(oracle_data),
            Token::FixedBytes(APP_DATA.to_vec()),
        ]),
    ]);

    // Prepend CoW AMM address to the signature so the settlement contract knows
    // which contract this signature refers to.
    let signature = cow_amm
        .address()
        .as_bytes()
        .iter()
        .cloned()
        .chain(signature_data)
        .collect::<Vec<_>>();

    // Create the commitment call for the pre-interaction
    let cow_amm_commitment = {
        let order_hash = cow_amm_order.hash_struct();
        let order_hash = hashed_eip712_message(&onchain.contracts().domain_separator, &order_hash);
        let commitment = cow_amm
            .commit(ethcontract::Bytes(order_hash))
            .tx
            .data
            .unwrap();
        Call {
            target: cow_amm.address(),
            value: 0.into(),
            calldata: commitment.0.to_vec(),
        }
    };

    // Fund trader "bob" with DAI and approve allowance
    dai.mint(bob.address(), to_wei(250)).await;

    dai.approve(
        onchain.contracts().allowance.into_alloy(),
        alloy::primitives::U256::MAX,
    )
    .from(bob.address().into_alloy())
    .send_and_watch()
    .await
    .unwrap();

    // Get balances before the trade
    let amm_weth_balance_before = onchain
        .contracts()
        .weth
        .balance_of(cow_amm.address())
        .call()
        .await
        .unwrap();
    let bob_weth_balance_before = onchain
        .contracts()
        .weth
        .balance_of(bob.address())
        .call()
        .await
        .unwrap();

    // Compensate a delay between the `CurrentBlockStream` and the actual onchain
    // data.
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Set the fees appropriately
    let fee_cow_amm = U256::exp10(16); // 0.01 WETH
    let fee_user = to_wei(1); // 1 DAI

    let mocked_solutions = |order_uid: OrderUid| {
        Solution {
            id: 1,
            prices: HashMap::from([
                (dai.address().into_legacy(), to_wei(1)), // 1 DAI = $1
                (onchain.contracts().weth.address(), to_wei(2300)), // 1 WETH = $2300
            ]),
            trades: vec![
                solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                    order: solvers_dto::solution::JitOrder {
                        sell_token: cow_amm_order.sell_token,
                        buy_token: cow_amm_order.buy_token,
                        receiver: cow_amm_order.receiver.unwrap_or_default(),
                        sell_amount: cow_amm_order.sell_amount,
                        buy_amount: cow_amm_order.buy_amount,
                        partially_fillable: cow_amm_order.partially_fillable,
                        valid_to: cow_amm_order.valid_to,
                        app_data: cow_amm_order.app_data.0,
                        kind: Kind::Sell,
                        sell_token_balance: SellTokenBalance::Erc20,
                        buy_token_balance: BuyTokenBalance::Erc20,
                        signing_scheme: SigningScheme::Eip1271,
                        signature: signature.clone(),
                    },
                    executed_amount: cow_amm_order.sell_amount - fee_cow_amm,
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
        }
    };

    // Configure the mocked `/quote` solver's solution
    let mocked_quote_solution = mocked_solutions(OrderUid([0u8; 56]));
    mock_solver.configure_solution(Some(mocked_quote_solution));

    let quote_request = OrderQuoteRequest {
        from: bob.address(),
        sell_token: dai.address().into_legacy(),
        buy_token: onchain.contracts().weth.address(),
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
    assert_eq!(quote_response.quote.sell_token, dai.address().into_legacy());
    assert_eq!(
        quote_response.quote.buy_token,
        onchain.contracts().weth.address()
    );
    // Ensure the amounts are the same as the solution proposes.
    assert_eq!(quote_response.quote.sell_amount, executed_amount);
    assert_eq!(quote_response.quote.buy_amount, U256::exp10(17));

    // Place user order where bob sells DAI to buy WETH (opposite direction)
    let user_order = OrderCreation {
        sell_token: dai.address().into_legacy(),
        sell_amount: executed_amount, // 230 DAI
        buy_token: onchain.contracts().weth.address(),
        buy_amount: U256::from(90000000000000000u64), // 0.09 WETH to generate some surplus
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(bob.private_key()).unwrap()),
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
            .balance_of(cow_amm.address())
            .call()
            .await
            .unwrap();
        let bob_weth_balance_after = onchain
            .contracts()
            .weth
            .balance_of(bob.address())
            .call()
            .await
            .unwrap();

        let amm_weth_sent = amm_weth_balance_before - amm_weth_balance_after;
        let bob_weth_received = bob_weth_balance_after - bob_weth_balance_before;

        // Bob should receive WETH, CoW AMM's WETH balance decreases
        bob_weth_received >= user_order.buy_amount && amm_weth_sent == cow_amm_order.sell_amount
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
