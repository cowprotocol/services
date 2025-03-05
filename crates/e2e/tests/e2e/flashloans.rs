use {
    contracts::{ERC20, IAavePool},
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{
            OnchainComponents,
            Services,
            TIMEOUT,
            run_forked_test_with_block_number,
            safe::Safe,
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
        signature::{Signature, hashed_eip712_message},
    },
    shared::{addr, conversions::U256Ext},
    std::time::Duration,
};

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

async fn forked_mainnet_repay_debt_with_collateral(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let trader = Safe::deploy(trader.clone(), &web3).await;

    // AAVE token tracking how much USDC is deposited by a user
    let ausdc = addr!("98c23e9d8f34fefb1b7bd6a91b7ff122f4e16f5c");
    let weth = &onchain.contracts().weth;
    let settlement = &onchain.contracts().gp_settlement;

    // transfer some USDC from a whale to our trader
    let usdc = ERC20::at(&web3, addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"));
    let usdc_whale_mainnet = addr!("28c6c06298d514db089934071355e5743bf21d60");
    let usdc_whale = forked_node_api
        .impersonate(&usdc_whale_mainnet)
        .await
        .unwrap();

    // transfer $50K + 1 atom to the safe (50K for the test and 1 atom for passing
    // the minimum balance test before placing the order)
    let collateral_amount = to_wei_with_exp(50_000, 6);
    tx!(
        usdc_whale,
        usdc.transfer(trader.address(), collateral_amount + U256::from(1))
    );
    // approve vault relayer to take safe's sell tokens
    trader
        .exec_call(usdc.approve(onchain.contracts().allowance, collateral_amount))
        .await;

    // Approve AAVE to take the collateral
    let aave_pool = IAavePool::at(&web3, addr!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"));
    trader
        .exec_call(usdc.approve(aave_pool.address(), collateral_amount))
        .await;

    // Deposit collateral
    trader
        .exec_call(aave_pool.supply(
            usdc.address(),    // token
            collateral_amount, // amount
            trader.address(),  // on_behalf
            0,                 // referral code
        ))
        .await;
    assert!(balance(&web3, trader.address(), ausdc).await >= collateral_amount);

    tracing::info!("wait a bit to make `borrow()` call work");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Borrow 1 WETH against the collateral
    let flashloan_amount = to_wei(1);
    trader
        .exec_call(aave_pool.borrow(
            onchain.contracts().weth.address(), // borrowed token
            flashloan_amount,                   // borrowed amount
            2.into(),                           // variable interest rate mode
            0,                                  // referral code
            trader.address(),                   // on_behalf
        ))
        .await;

    // allow aave pool to take back borrowed WETH on `repay()`
    // could be replaced with `permit` pre-hook or `repayWithPermit()` for
    // borrowed tokens that support `permit`
    trader
        .exec_call(
            onchain
                .contracts()
                .weth
                .approve(aave_pool.address(), to_wei(1)),
        )
        .await;

    let current_safe_nonce = trader.nonce().await;

    // Build appdata that does:
    // 1. take out a 1 WETH flashloan for the trader
    // 2. repay the 1 WETH debt to unlock trader's collateral (1st pre-hook)
    // 3. withdraw the collateral it can be sold for WETH (2nd pre-hook)
    let app_data = {
        let repay_tx = trader.sign_transaction(
            aave_pool.address(),
            aave_pool
                .repay(
                    onchain.contracts().weth.address(),
                    to_wei(1),
                    2.into(),
                    trader.address(),
                )
                .tx
                .data
                .unwrap()
                .0
                .clone(),
            current_safe_nonce,
        );
        let withdraw_tx = trader.sign_transaction(
            aave_pool.address(),
            aave_pool
                .withdraw(usdc.address(), collateral_amount, trader.address())
                .tx
                .data
                .unwrap()
                .0
                .clone(),
            current_safe_nonce + U256::from(1),
        );
        let app_data = format!(
            r#"{{
                "metadata": {{
                    "flashloan": {{
                        "lender": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
                        "token": "{:?}",
                        "amount": "{:?}"
                    }},
                    "hooks": {{
                        "pre": [
                            {{
                                "target": "{:?}",
                                "value": "0",
                                "callData": "0x{}",
                                "gasLimit": "1000000"
                            }},
                            {{
                                "target": "{:?}",
                                "value": "0",
                                "callData": "0x{}",
                                "gasLimit": "1000000"
                            }}
                        ],
                        "post": []
                    }},
                    "signer": "{:?}"
                }}
            }}"#,
            // flashloan
            onchain.contracts().weth.address(),
            // take out a loan that's bigger than we originally borrowed
            flashloan_amount,
            // 1st pre-hook
            trader.address(),
            hex::encode(&repay_tx.tx.data.unwrap().0),
            // 2nd pre-hook
            // ~200K gas
            trader.address(),
            hex::encode(&withdraw_tx.tx.data.unwrap().0),
            // signer
            trader.address(),
        );

        OrderCreationAppData::Full {
            full: app_data.to_string(),
        }
    };

    // pay 9 bps of flashloan fee to the AAVE pool
    let flashloan_fee = (flashloan_amount * U256::from(9)).ceil_div(&10_000.into());
    let mut order = OrderCreation {
        sell_token: usdc.address(),
        sell_amount: collateral_amount,
        buy_token: onchain.contracts().weth.address(),
        buy_amount: flashloan_amount + flashloan_fee,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        // Receiver is always the settlement contract, so driver will have to manually send funds to
        // solver wrapper (flashloan borrower)
        receiver: Some(onchain.contracts().gp_settlement.address()),
        ..Default::default()
    };
    order.signature = Signature::Eip1271(trader.sign_message(&hashed_eip712_message(
        &onchain.contracts().domain_separator,
        &order.data().hash_struct(),
    )));

    {
        let settlement = forked_node_api
            .impersonate(&settlement.address())
            .await
            .unwrap();
        let amount = balance(&web3, settlement.address(), weth.address()).await;
        tx!(settlement, weth.transfer(H160([1; 20]), amount,));
        let amount = balance(&web3, settlement.address(), weth.address()).await;
        assert_eq!(amount, 0.into());

        let amount = balance(&web3, settlement.address(), usdc.address()).await;
        tx!(settlement, usdc.transfer(H160([1; 20]), amount,));
        let amount = balance(&web3, settlement.address(), weth.address()).await;
        assert_eq!(amount, 0.into());
    }
    tracing::info!("Removed all USDC and WETH from settlement contract");

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    assert_eq!(
        balance(&web3, trader.address(), usdc.address()).await,
        1.into()
    );
    tracing::info!("trader just has 1 atom of USDC before placing order");

    let uid = services.create_order(&order).await.unwrap();
    tracing::info!(?uid, "placed order");

    // Drive solution
    tracing::info!("Waiting for trade to get indexed.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let executed_fee = services
            .get_order(&uid)
            .await
            .unwrap()
            .metadata
            .executed_fee;
        executed_fee > 0.into()
    })
    .await
    .unwrap();

    let trader_usdc = balance(&web3, trader.address(), usdc.address()).await;
    assert!(trader_usdc > to_wei_with_exp(47_000, 6));
    tracing::info!("trader got majority of collateral back");

    let settlement_weth = balance(&web3, settlement.address(), weth.address()).await;
    assert!(settlement_weth < 200_000_000u128.into());
    tracing::info!("settlement contract only has dust amounts of WETH");

    assert!(balance(&web3, trader.address(), ausdc).await < 1_000.into());
    tracing::info!("trader only has dust of aUSDC");
}

async fn balance(web3: &Web3, owner: H160, token: H160) -> U256 {
    let token = ERC20::at(web3, token);
    token.balance_of(owner).call().await.unwrap()
}
