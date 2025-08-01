use {
    contracts::{ERC20, IAavePool},
    database::Address,
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{
            Db,
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
    sqlx::Row,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_repay_debt_with_collateral_of_safe() {
    run_forked_test_with_block_number(
        forked_mainnet_repay_debt_with_collateral_of_safe,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        // https://etherscan.io/tx/0x215de2ddda2e16bf6d21d148a6c1519e94a4eee047ddd100778e01ee6ba0cf2a
        23031384,
    )
    .await;
}

// Tests the rough flow of how a safe that took out a loan on AAVE
// could repay it using its own collateral fronted by a flashloan.
async fn forked_mainnet_repay_debt_with_collateral_of_safe(web3: Web3) {
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
    // 1. take out a 1 WETH flashloan for the trader (flashloan hint)
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

    // pay some extra for the flashloan fee
    let fee_bps = aave_pool.flashloan_premium_total().call().await.unwrap();
    let flashloan_fee = (flashloan_amount * U256::from(fee_bps)).ceil_div(&10_000.into());
    let mut order = OrderCreation {
        sell_token: usdc.address(),
        sell_amount: collateral_amount,
        buy_token: onchain.contracts().weth.address(),
        buy_amount: flashloan_amount + flashloan_fee,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        // receiver is always the settlement contract because the driver takes
        // funds from the settlement contract to pay back the loan
        receiver: Some(onchain.contracts().gp_settlement.address()),
        ..Default::default()
    };
    order.signature = Signature::Eip1271(trader.sign_message(&hashed_eip712_message(
        &onchain.contracts().domain_separator,
        &order.data().hash_struct(),
    )));

    tracing::info!("Removing all USDC and WETH from settlement contract for easier accounting");
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

    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

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

    // Because the trader sold some of their collateral to repay their debt
    // (~3900 USDC for ~1 WETH) they have that much less `USDC` compared to
    // the original collateral.
    let trader_usdc = balance(&web3, trader.address(), usdc.address()).await;
    assert!(trader_usdc > to_wei_with_exp(46_000, 6));
    tracing::info!("trader got majority of collateral back");

    let settlement_weth = balance(&web3, settlement.address(), weth.address()).await;
    assert!(settlement_weth < 300_000_000u128.into());
    tracing::info!("settlement contract only has dust amounts of WETH");

    assert!(balance(&web3, trader.address(), ausdc).await < 10_000.into());
    tracing::info!("trader only has dust of aUSDC");

    // Check that the solver address is stored in the settlement table
    let pool = services.db();
    let settler_is_solver = || async {
        let last_solver = fetch_last_settled_auction_solver(pool).await;
        last_solver.is_some_and(|address| address.0 == solver.address().0)
    };
    wait_for_condition(TIMEOUT, settler_is_solver)
        .await
        .unwrap();
}

async fn balance(web3: &Web3, owner: H160, token: H160) -> U256 {
    let token = ERC20::at(web3, token);
    token.balance_of(owner).call().await.unwrap()
}

async fn fetch_last_settled_auction_solver(pool: &Db) -> Option<Address> {
    sqlx::query("SELECT solver FROM settlements ORDER BY auction_id DESC")
        .fetch_all(pool)
        .await
        .unwrap()
        .first()
        .map(|row| row.try_get(0).unwrap())
}
