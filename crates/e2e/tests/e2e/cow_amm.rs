use {
    app_data::AppDataHash,
    e2e::{
        setup::{colocation::SolverEngine, mock::Mock, *},
        tx,
        tx_value,
    },
    ethcontract::{web3::ethabi::Token, BlockId, BlockNumber, U256},
    model::{
        order::{OrderCreation, OrderData, OrderKind},
        signature::{hashed_eip712_message, EcdsaSigningScheme},
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    solvers_dto::solution::{
        BuyTokenBalance,
        Call,
        Kind,
        SellTokenBalance,
        SigningScheme,
        Solution,
    },
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
    tx!(
        cow_amm_owner.account(),
        dai.approve(cow_amm_factory.address(), to_wei(2_000))
    );
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
        .get_pair(onchain.contracts().weth.address(), dai.address())
        .call()
        .await
        .expect("failed to get Uniswap V2 pair");

    let cow_amm = cow_amm_factory
        .amm_deterministic_address(
            cow_amm_owner.address(),
            dai.address(),
            onchain.contracts().weth.address(),
        )
        .call()
        .await
        .unwrap();

    let oracle_data: Vec<_> = std::iter::repeat(0u8)
        .take(12) // pad with 12 zeros in the front to end up with 32 bytes
        .chain(pair.as_bytes().to_vec())
        .collect();
    const APP_DATA: [u8; 32] = [12u8; 32];

    cow_amm_factory
        .create(
            dai.address(),
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
    let services = Services::new(onchain.contracts()).await;
    services
        .start_autopilot(
            None,
            vec![
                "--drivers=mock_solver|http://localhost:11088/mock_solver".to_string(),
                "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver"
                    .to_string(),
                format!("--protocol-fee-exempt-addresses={:?}", cow_amm.address()),
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
        buy_token: dai.address(),
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
                hex::decode("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775")
                    .unwrap(),
            ), // sell order
            Token::Bool(cow_amm_order.partially_fillable),
            Token::FixedBytes(
                hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9")
                    .unwrap(),
            ), // sell_token_source == erc20
            Token::FixedBytes(
                hex::decode("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9")
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
        buy_token: dai.address(),
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

    let amm_balance_before = dai.balance_of(cow_amm.address()).call().await.unwrap();
    let bob_balance_before = dai.balance_of(bob.address()).call().await.unwrap();

    let fee = U256::exp10(16); // 0.01 WETH

    mock_solver.configure_solution(Some(Solution {
        id: 1,
        // assume price of the univ2 pool
        prices: HashMap::from([
            (dai.address(), to_wei(100)),
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
                order: user_order_id.0,
                executed_amount: user_order.sell_amount - fee,
                fee: Some(fee),
            }),
        ],
        pre_interactions: vec![cow_amm_commitment],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
    }));

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let amm_balance = dai.balance_of(cow_amm.address()).call().await.unwrap();
        let bob_balance = dai.balance_of(bob.address()).call().await.unwrap();

        let amm_received = amm_balance - amm_balance_before;
        let bob_received = bob_balance - bob_balance_before;

        // bob and CoW AMM both got surplus and an equal amount
        amm_received > cow_amm_order.buy_amount
            && bob_received > user_order.buy_amount
            && amm_received == bob_received
    })
    .await
    .unwrap();
}
