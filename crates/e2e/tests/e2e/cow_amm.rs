use {
    app_data::AppDataHash,
    e2e::{
        setup::{colocation::SolverEngine, *},
        tx,
        tx_value,
    },
    ethcontract::{web3::ethabi::Token, U256},
    model::{
        order::{OrderCreation, OrderData, OrderKind},
        signature::{hashed_eip712_message, EcdsaSigningScheme},
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    solvers_dto::solution::{BuyTokenBalance, Call, Kind, SellTokenBalance, SigningScheme, Solution},
    std::collections::HashMap,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_cow_amm() {
    run_test(cow_amm).await;
}

async fn cow_amm(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(100)).await;
    let [bob, cow_amm_owner, helper] = onchain.make_accounts(to_wei(1000)).await;

    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(300_000), to_wei(100))
        .await;

    // Temporarily fund the contract with lots of money to debug reverts.
    dai.mint(onchain.contracts().gp_settlement.address(), to_wei(100_000_000)).await;
    tx_value!(
        helper.account(),
        to_wei(999),
        onchain.contracts().weth.deposit()
    );
    tx!(
        helper.account(),
        onchain
            .contracts()
            .weth
            .transfer(onchain.contracts().allowance, to_wei(999))
    );

    // fund trader "bob" and approve vault relayer
    tx_value!(bob.account(), to_wei(1), onchain.contracts().weth.deposit());
    tx!(
        bob.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, U256::MAX)
    );

    let oracle = contracts::CowAmmUniswapV2PriceOracle::builder(&web3)
        .deploy()
        .await
        .unwrap();

    // set up cow_amm
    let cow_amm_factory = contracts::CowAmmConstantProductFactory::builder(
        &web3,
        onchain.contracts().gp_settlement.address(),
    )
    .deploy()
    .await
    .unwrap();

    // Fund cow amm owner with 10 `token` and allow factory take them
    dai.mint(cow_amm_owner.address(), to_wei(2_000)).await;
    tx!(
        cow_amm_owner.account(),
        dai.approve(cow_amm_factory.address(), to_wei(2_000))
    );
    // Fund cow amm owner with 10 WETH and allow factory take them
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

    // univ2 pair address encoded as 32 bytes
    let pair = onchain
        .contracts()
        .uniswap_v2_factory
        .get_pair(onchain.contracts().weth.address(), dai.address())
        .call()
        .await
        .expect("failed to get Uniswap V2 pair");

    let oracle_data = ethcontract::web3::ethabi::encode(&[Token::Address(pair)]);

    let cow_amm = cow_amm_factory
        .amm_deterministic_address(
            cow_amm_owner.address(),
            dai.address(),
            onchain.contracts().weth.address(),
        )
        .call()
        .await
        .unwrap();

    cow_amm_factory
        .create(
            dai.address(),
            to_wei(2_000),
            onchain.contracts().weth.address(),
            to_wei(1),
            0.into(), // min traded token
            oracle.address(),
            ethcontract::Bytes(oracle_data.clone()),
            ethcontract::Bytes([0; 32]), // appdata
        )
        .from(cow_amm_owner.account().clone())
        .send()
        .await
        .unwrap();
    let cow_amm = contracts::CowAmm::at(&web3, cow_amm);

    let mock_solver = Mock::default();

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            SolverEngine {
                name: "test_solver".into(),
                account: solver.clone(),
                endpoint: colocation::start_baseline_solver(onchain.contracts().weth.address())
                    .await,
            },
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    // We start the quoter as the baseline solver, and the mock solver as the one
    // returning the solution
    let services = Services::new(onchain.contracts()).await;
    services
        .start_autopilot(
            None,
            vec![
                "--drivers=mock_solver|http://localhost:11088/mock_solver".to_string(),
                "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver"
                    .to_string(),
                format!("--protocol-fee-exempt-addresses={:?}", cow_amm.address())
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        ])
        .await;
    tracing::error!(cow_amm = ?cow_amm.address());

    // place user order
    let user_order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: to_wei(1),
        buy_token: dai.address(),
        buy_amount: to_wei(100),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(bob.private_key()).unwrap()),
    );
    let user_order_id = services.create_order(&user_order).await.unwrap();

    let encoded_trading_params = ethcontract::web3::ethabi::encode(&[
        Token::Uint(0.into()), // min_traded_token
        Token::Address(oracle.address()),
        Token::Bytes(oracle_data),
        Token::FixedBytes([0u8; 32].to_vec()),
    ]);

    let cow_amm_order = OrderData {
        sell_token: dai.address(),
        buy_token: onchain.contracts().weth.address(),
        receiver: None,
        sell_amount: to_wei(100),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        app_data: AppDataHash([0u8; 32]),
        fee_amount: 0.into(),
        kind: OrderKind::Sell,
        partially_fillable: false,
        sell_token_balance: Default::default(),
        buy_token_balance: Default::default(),
    };

    let encoded_cow_amm_order = ethcontract::web3::ethabi::encode(&[
        Token::Address(cow_amm_order.sell_token),
        Token::Address(cow_amm_order.buy_token),
        Token::Address(cow_amm_order.receiver.unwrap_or_default()),
        Token::Uint(cow_amm_order.sell_amount),
        Token::Uint(cow_amm_order.buy_amount),
        Token::Uint(cow_amm_order.valid_to.into()),
        Token::FixedBytes(cow_amm_order.app_data.0.to_vec()),
        Token::Uint(cow_amm_order.fee_amount),
        Token::FixedBytes(hex::decode("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775").unwrap()), // sell order
        Token::Bool(cow_amm_order.partially_fillable),
        Token::FixedBytes(hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9").unwrap()), // sell_token_source
        Token::FixedBytes(hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9").unwrap()), // erc20 buy_token_destination
    ]);

    let signature_data = ethcontract::web3::ethabi::encode(&[
        Token::Bytes(encoded_cow_amm_order),
        Token::Bytes(encoded_trading_params),
    ]);
    let signature = cow_amm.address().as_bytes().iter().cloned().chain(signature_data).collect();

    let pre_interaction = {
        let order_hash = cow_amm_order.hash_struct();
        let order_hash = hashed_eip712_message(&onchain.contracts().domain_separator, &order_hash);
        let readable_hash = hex::encode(order_hash);
        tracing::error!(?readable_hash, "commited order hash");
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

    // todo generate cow amm order
    // ConstantProduct.TradingParams memory data = ConstantProduct.TradingParams({
    //     minTradedToken0: minTradedToken0,
    //     priceOracle: uniswapV2PriceOracle, // address
    //     priceOracleData: priceOracleData, // bytes
    //     appData: appData // appdata
    // });
    // GPv2Order.Data memory order = GPv2Order.Data({
    //     sellToken: DAI,
    //     buyToken: WETH,
    //     receiver: GPv2Order.RECEIVER_SAME_AS_OWNER,
    //     sellAmount: sellAmount,
    //     buyAmount: buyAmount,
    //     validTo: latestValidTimestamp,
    //     appData: appData,
    //     feeAmount: 0,
    //     kind: GPv2Order.KIND_SELL,
    //     partiallyFillable: true,
    //     sellTokenBalance: GPv2Order.BALANCE_ERC20,
    //     buyTokenBalance: GPv2Order.BALANCE_ERC20
    // });
    // bytes memory sig = abi.encode(order, data);

    // bytes32 domainSeparator = settlement.domainSeparator();
    // // The commit should be part of the settlement for the test to work.
    // // This would require us to vendor quite a lot of helper code from
    // // composable-cow to include interactions in `settle`. For now, we
    // // rely on the fact that Foundry doesn't reset transient storage
    // // between calls.
    // vm.prank(address(settlement));
    // amm.commit(order.hash(domainSeparator));
    // settle(address(amm), bob, order, sig, hex"");

    mock_solver.configure_solution(Some(Solution {
        id: 1,
        prices: HashMap::from([
            (dai.address(), to_wei(1)),
            (onchain.contracts().weth.address(), to_wei(1_000)),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                order: solvers_dto::solution::JitOrder {
                    sell_token: cow_amm_order.sell_token,
                    buy_token: cow_amm_order.buy_token,
                    receiver: cow_amm_order.receiver.unwrap_or_default(),
                    sell_amount: cow_amm_order.sell_amount,
                    buy_amount: cow_amm_order.buy_amount,
                    valid_to: cow_amm_order.valid_to,
                    app_data: cow_amm_order.app_data.0,
                    fee_amount: cow_amm_order.fee_amount,
                    kind: Kind::Sell,
                    partially_fillable: cow_amm_order.partially_fillable,
                    sell_token_balance: SellTokenBalance::Erc20,
                    buy_token_balance: BuyTokenBalance::Erc20,
                    signing_scheme: SigningScheme::Eip1271,
                    signature,
                },
                executed_amount: cow_amm_order.sell_amount,
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: user_order.buy_amount,
                fee: Some(0.into()),
                order: user_order_id.0,
            }),
        ],
        pre_interactions: vec![pre_interaction],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
    }));

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance = dai.balance_of(bob.address()).call().await.unwrap();
        balance >= to_wei(100)
    })
    .await
    .unwrap();

    // let signature1271 = safe.order_eip1271_signature(&order_template,
    // &onchain);

    // // Check that we can't place invalid orders.
    // let orders = [
    //     OrderCreation {
    //         from: Some(safe.address()),
    //         signature: Signature::Eip1271(b"invalid signature".to_vec()),
    //         ..order_template.clone()
    //     },
    //     OrderCreation {
    //         from: Some(H160(*b"invalid address\0\0\0\0\0")),
    //         signature: Signature::Eip1271(signature1271.clone()),
    //         ..order_template.clone()
    //     },
    // ];
    // for order in &orders {
    //     let (_, err) = dbg!(services.create_order(order).await.unwrap_err());
    //     assert!(err.contains("InvalidEip1271Signature"));
    // }

    // // Place orders
    // let orders = [
    //     OrderCreation {
    //         from: Some(safe.address()),
    //         signature: Signature::Eip1271(signature1271),
    //         ..order_template.clone()
    //     },
    //     OrderCreation {
    //         app_data: OrderCreationAppData::Full {
    //             full: "{\"salt\": \"second\"}".to_string(),
    //         },
    //         from: Some(safe.address()),
    //         signature: Signature::PreSign,
    //         ..order_template.clone()
    //     },
    // ];

    // let mut uids = Vec::new();
    // for order in &orders {
    //     let uid = services.create_order(order).await.unwrap();
    //     uids.push(uid);
    // }
    // let uids = uids;

    // let order_status = |order_uid: OrderUid| {
    //     let services = &services;
    //     async move {
    //         services
    //             .get_order(&order_uid)
    //             .await
    //             .unwrap()
    //             .metadata
    //             .status
    //     }
    // };

    // // Check that the EIP-1271 order was received.
    // assert_eq!(order_status(uids[0]).await, OrderStatus::Open);

    // // Execute pre-sign transaction.
    // assert_eq!(
    //     order_status(uids[1]).await,
    //     OrderStatus::PresignaturePending
    // );
    // safe.exec_call(
    //     onchain
    //         .contracts()
    //         .gp_settlement
    //         .set_pre_signature(Bytes(uids[1].0.to_vec()), true),
    // )
    // .await;

    // // Check that the presignature event was received.
    // wait_for_condition(TIMEOUT, || async {
    //     services.get_auction().await.auction.orders.len() == 2
    // })
    // .await
    // .unwrap();
    // assert_eq!(order_status(uids[1]).await, OrderStatus::Open);

    // // Drive solution
    // tracing::info!("Waiting for trade.");
    // wait_for_condition(TIMEOUT, || async {
    //     services.get_auction().await.auction.orders.is_empty()
    // })
    // .await
    // .unwrap();

    // // Check matching
    // let balance = token
    //     .balance_of(safe.address())
    //     .call()
    //     .await
    //     .expect("Couldn't fetch token balance");
    // assert_eq!(balance, U256::zero());

    // let balance = onchain
    //     .contracts()
    //     .weth
    //     .balance_of(safe.address())
    //     .call()
    //     .await
    //     .expect("Couldn't fetch native token balance");
    // assert!(balance > to_wei(6));
}
