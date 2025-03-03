use {
    alloy::{
        dyn_abi::Eip712Domain,
        signers::{SignerSync, local::PrivateKeySigner},
        sol_types::{SolCall, SolStruct, SolValue},
    },
    contracts::{COWShedFactory, ERC20, IAavePool},
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{
            OnchainComponents,
            Services,
            TIMEOUT,
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
        },
        tx,
    },
    ethcontract::{H160, U256},
    ethrpc::Web3,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    serde::Serialize,
    shared::addr,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_single_flashloan_encoding_maker() {
    run_forked_test_with_block_number(
        forked_mainnet_single_flashloan_encoding_maker_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        21874126,
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_single_flashloan_encoding_aave() {
    run_forked_test_with_block_number(
        forked_mainnet_single_flashloan_encoding_aave_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        21874126,
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_repay_debt_with_collateral() {
    run_forked_test_with_block_number(
        forked_mainnet_repay_debt_with_collateral,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        21874126,
    )
    .await;
}

async fn forked_mainnet_single_flashloan_encoding_maker_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_dai = ERC20::at(
        &web3,
        "0x6B175474E89094C44Da98b954EedeAC495271d0F"
            .parse()
            .unwrap(),
    );

    // find some USDC available onchain
    const USDC_WHALE_MAINNET: H160 = H160(hex_literal::hex!(
        "28c6c06298d514db089934071355e5743bf21d60"
    ));
    // Give trader some USDC
    let usdc_whale = forked_node_api
        .impersonate(&USDC_WHALE_MAINNET)
        .await
        .unwrap();
    tx!(
        usdc_whale,
        token_usdc.transfer(trader.address(), to_wei_with_exp(1000, 6))
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, to_wei_with_exp(1000, 6))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    // App data with flashloan
    let app_data = format!(
        r#"{{
        "metadata": {{
            "flashloan": {{
                "lender": "0x60744434d6339a6B27d73d9Eda62b6F66a0a04FA",
                "borrower": "{:?}",
                "token": "0x6B175474E89094C44Da98b954EedeAC495271d0F",
                "amount": "900000000000000000000"
            }}
        }}
    }}"#,
        trader.address()
    );

    let app_data = OrderCreationAppData::Full {
        full: app_data.to_string(),
    };

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei_with_exp(1000, 6),
        buy_token: token_dai.address(),
        buy_amount: to_wei_with_exp(900, 18), // equal to flashloan amount
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        // Receiver is always the settlement contract, so driver will have to manually send funds to
        // solver wrapper (flashloan borrower)
        receiver: Some(onchain.contracts().gp_settlement.address()),
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
            sell_token: token_usdc.address(),
            buy_token: token_dai.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(1000, 6).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;
    let order_id = services.create_order(&order).await.unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;

        let executed_fee = services
            .get_order(&order_id)
            .await
            .unwrap()
            .metadata
            .executed_fee;
        executed_fee > 0.into()

        // TODO balances
    })
    .await
    .unwrap();
}

async fn forked_mainnet_single_flashloan_encoding_aave_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_weth = ERC20::at(
        &web3,
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .unwrap(),
    );

    // find some USDC available onchain
    const USDC_WHALE_MAINNET: H160 = H160(hex_literal::hex!(
        "28c6c06298d514db089934071355e5743bf21d60"
    ));
    // Give trader some USDC
    let usdc_whale = forked_node_api
        .impersonate(&USDC_WHALE_MAINNET)
        .await
        .unwrap();
    tx!(
        usdc_whale,
        token_usdc.transfer(trader.address(), to_wei_with_exp(50000, 6))
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, to_wei_with_exp(50000, 6))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    // App data with flashloan
    let app_data = format!(
        r#"{{
        "metadata": {{
            "flashloan": {{
                "lender": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
                "borrower": "{:?}",
                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "amount": "5000000000000000000"
            }}
        }}
    }}"#,
        trader.address()
    );

    let app_data = OrderCreationAppData::Full {
        full: app_data.to_string(),
    };

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei_with_exp(50000, 6),
        buy_token: token_weth.address(),
        buy_amount: U256::from(5005000000000000000u128), // equal to flashloan amount + 0.1% fee
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        // Receiver is always the settlement contract, so driver will have to manually send funds to
        // solver wrapper (flashloan borrower)
        receiver: Some(onchain.contracts().gp_settlement.address()),
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
            sell_token: token_usdc.address(),
            buy_token: token_weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(50000, 6).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;
    let order_id = services.create_order(&order).await.unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;

        let executed_fee = services
            .get_order(&order_id)
            .await
            .unwrap()
            .metadata
            .executed_fee;
        executed_fee > 0.into()

        // TODO balances
    })
    .await
    .unwrap();
}

async fn forked_mainnet_repay_debt_with_collateral(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;

    // transfer some USDC from a whale to our trader
    let usdc = ERC20::at(&web3, addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"));
    let usdc_whale_mainnet = addr!("28c6c06298d514db089934071355e5743bf21d60");
    let usdc_whale = forked_node_api
        .impersonate(&usdc_whale_mainnet)
        .await
        .unwrap();
    tx!(
        usdc_whale,
        usdc.transfer(trader.address(), to_wei_with_exp(50000, 6))
    );

    let aave_pool = IAavePool::at(&web3, addr!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"));

    // Approve AAVE to take the collateral
    tx!(
        trader.account(),
        usdc.approve(aave_pool.address(), to_wei_with_exp(50000, 6))
    );
    // Deposit 50K USDC as collateral
    tx!(
        trader.account(),
        aave_pool.supply(
            usdc.address(),             // token
            to_wei_with_exp(50_000, 6), // amount
            trader.address(),           // on_behalf
            0,                          // referral code
        )
    );

    // Borrow 1 WETH against the USDC
    tx!(
        trader.account(),
        aave_pool.borrow(
            onchain.contracts().weth.address(), // borrowed token
            to_wei(18),                         // borrowed amount
            2.into(),                           // variable interest rate mode
            0,                                  // referral code
            trader.address(),                   // on_behalf
        )
    );

    // do a bunch of stuff to build the appdata that:
    // 1. takes out an 1 WETH flashloan for the user's cowshed proxy (flashloan
    //    metadata)
    // 2. repays the user's 1 WETH debt to unlock their 50K USDC (1st pre-hook)
    // 3. withdraws the deposited 50K USDC for the user so they can sell it for WETH
    //    (2nd pre-hook) currently only the order owner can withdraw the tokens
    //    which would require a helper contract to make this a permissionless
    //    operation that can be called in a pre-hook
    let app_data = {
        let alloy_trader: PrivateKeySigner = hex::encode(trader.private_key()).parse().unwrap();
        let cowshed_factory =
            COWShedFactory::at(&web3, addr!("00E989b87700514118Fa55326CD1cCE82faebEF6"));
        // compute cowshed proxy for trader
        let cowshed_proxy = cowshed_factory
            .proxy_of(trader.address())
            .call()
            .await
            .unwrap();

        let hooks = COWShedHooks {
            nonce: Default::default(),
            deadline: alloy::primitives::U256::MAX,
            calls: vec![Call {
                target: aave_pool.address().0.into(),
                value: alloy::primitives::U256::ZERO,
                callData: repayCall {
                    asset: onchain.contracts().weth.address().0.into(),
                    amount: alloy::primitives::utils::parse_ether("1").unwrap(),
                    interestRateMode: alloy::primitives::U256::from(2),
                    onBehalfOf: alloy_trader.address(),
                }
                .abi_encode()
                .into(),
                allowFailure: false,
                isDelegateCall: false,
            }],
        };
        let domain = cowshed_proxy_domain_separator(cowshed_proxy, 1);
        let hash_to_sign = hooks.eip712_signing_hash(&domain);
        let signature = alloy_trader.sign_hash_sync(&hash_to_sign).unwrap();
        // TODO: check if `v` being a bool when it should be u8 causes issues
        let signature = (signature.r(), signature.s(), signature.v()).abi_encode_packed();
        let factory_hook_call = ICOWShedFactory::executeHooksCall {
            user: alloy_trader.address(),
            calls: hooks.calls,
            deadline: hooks.deadline,
            nonce: hooks.nonce,
            signature: signature.into(),
        }
        .abi_encode();

        // TODO: check if the `lender` is actually correct of if it should
        // be the Aave pool we have been using throughout the test.
        let app_data = format!(
            r#"{{
                "metadata": {{
                    "flashloan": {{
                        "lender": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
                        "borrower": "{cowshed_proxy:?}",
                        "token": "{:?}",
                        "amount": "50000000000"
                    }},
                    "hooks": {{
                        "target": {:?},
                        "value": "0",
                        "callData": {:?}
                    }}
                }}
            }}"#,
            usdc.address(),
            cowshed_factory.address(),
            hex::encode(&factory_hook_call),
        );

        OrderCreationAppData::Full {
            full: app_data.to_string(),
        }
    };

    let order = OrderCreation {
        sell_token: usdc.address(),
        sell_amount: to_wei_with_exp(50000, 6),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        // Receiver is always the settlement contract, so driver will have to manually send funds to
        // solver wrapper (flashloan borrower)
        receiver: Some(onchain.contracts().gp_settlement.address()),
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    // flow of funds:
    // 1. user borrows funds on AAVE
    // 2. flashloan goes to cowshed
    // 3. cowshed repays debt position this step should
    // 4. HOW DOES USER ACTUALLY WITHDRAW THE COLLATERAL TOKENS??
    // 5. used executes trade `COLLATERAL => BORROWED_TOKEN`
    // 6. user pays settlement contract

    panic!("abort for now");
}

fn cowshed_proxy_domain_separator(proxy: H160, chain_id: u64) -> Eip712Domain {
    alloy::sol_types::eip712_domain! {
        name: "COWShed",
        version: "1.0.0",
        chain_id: chain_id,
        verifying_contract: proxy.0.into(),
    }
}

alloy::sol! {
    #[derive(Serialize)]
    struct COWShedHooks {
        Call[] calls;
        bytes32 nonce;
        uint256 deadline;
    }

    #[derive(Serialize)]
    struct Call {
        address target;
        uint256 value;
        bytes callData;
        bool allowFailure;
        bool isDelegateCall;
    }

    #[derive(Serialize)]
    contract ICOWShedFactory {
        function executeHooks(
            Call[] calldata calls,
            bytes32 nonce,
            uint256 deadline,
            address user,
            bytes calldata signature
        ) external;
    }

    function repay(
      address asset,
      uint256 amount,
      uint256 interestRateMode,
      address onBehalfOf
    ) external returns (uint256);
}
