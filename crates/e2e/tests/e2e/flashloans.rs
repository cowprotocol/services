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
        23112197,
    )
    .await;
}

// Tests the rough flow of how a safe that took out a loan on AAVE
// could repay it using its own collateral fronted by a flashloan.
// Example: you put up some collateral (here 50k USDC) and you take out a
// loan (1 weth) against it. If you spend whatever you borrowed you are still
// solvent, but can't use the collateral anymore, because the contract requires
// you pay back the loan first (but you spent the 1 weth already). The use-case
// for flash loans here is to take out a flash loan, pay back the original loan
// with it, which unlocks your 50k USDC collateral, swap (some) of your
// the collateral for the loan amount (1 weth + e.g. 0.05% fee) and pay back the
// flash loan. Voila, for a small fee you got to use your collateral pay back
// your loan and unlock the rest.
async fn forked_mainnet_repay_debt_with_collateral_of_safe(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let trader = Safe::deploy(trader.clone(), &web3).await;

    let aave_adapter_contract = addr!("7d9c4dee56933151bc5c909cfe09def0d315cb4a");

    // AAVE token tracking how much USDC is deposited by a user
    let ausdc = addr!("98c23e9d8f34fefb1b7bd6a91b7ff122f4e16f5c");
    let weth = &onchain.contracts().weth;
    let tracker_address = onchain
        .contracts()
        .flashloan_tracker
        .as_ref()
        .expect("tracker")
        .address();

    // transfer some USDC from a whale to our trader
    let usdc = ERC20::at(&web3, addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"));
    let usdc_whale_mainnet = addr!("28c6c06298d514db089934071355e5743bf21d60");
    let usdc_whale = forked_node_api
        .impersonate(&usdc_whale_mainnet)
        .await
        .unwrap();

    // transfer $50K to the safe for setting up the debt position
    let collateral_amount = to_wei_with_exp(50_000, 6);
    tx!(
        usdc_whale,
        usdc.transfer(trader.address(), collateral_amount)
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

    // Exchange rate between USDC and aUSDC can differ from block to block.
    let slippage = 2;
    assert!(balance(&web3, trader.address(), ausdc).await >= collateral_amount - slippage);

    tracing::info!("wait a bit to make `borrow()` call work");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Borrow 1 WETH against the collateral - not a flash loan
    let debt_amount = to_wei(1);
    trader
        .exec_call(aave_pool.borrow(
            onchain.contracts().weth.address(), // borrowed token
            debt_amount,                        // borrowed amount
            2.into(),                           // variable interest rate mode
            0,                                  // referral code
            trader.address(),                   // on_behalf
        ))
        .await;

    // Transfer the borrowed WETH to /dev/null to make it clear this is not used for
    // repayment. Let's imagine the trader used this to pay their rent or something.
    trader
        .exec_call(
            onchain
                .contracts()
                .weth
                .transfer(H160([1; 20]), debt_amount),
        )
        .await;

    // allow aave pool to take back borrowed WETH on `repay()`
    // could be replaced with `permit` pre-hook or `repayWithPermit()` for
    // borrowed tokens that support `permit`
    trader
        .exec_call(weth.approve(aave_pool.address(), to_wei(1)))
        .await;

    tracing::info!("tracker contract address: {:?}", tracker_address);
    // allow flashloan tracker to take back flash loan WETH on `payBack()`
    trader
        .exec_call(weth.approve(tracker_address, to_wei(1)))
        .await;

    // pay some extra for the flashloan fee
    let fee_bps = aave_pool.flashloan_premium_total().call().await.unwrap();
    let flashloan_fee = (debt_amount * U256::from(fee_bps)).ceil_div(&10_000.into());

    let current_safe_nonce = trader.nonce().await;

    // Build appdata that does:
    // 1. take out a 1 WETH flashloan for the trader (flashloan hint)
    // 2. repay the 1 WETH debt to unlock trader's collateral (1st pre-hook)
    // 3. withdraw the collateral it can be sold for WETH (2nd pre-hook)
    // 4. repay the flashloan with the proceeds (1st post-hook)
    let app_data = {
        let repay_debt_tx = trader.sign_transaction(
            aave_pool.address(),
            aave_pool
                .repay(weth.address(), to_wei(1), 2.into(), trader.address())
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

        let repay_flashloan_fee = trader.sign_transaction(
            weth.address(),
            weth.transfer(aave_adapter_contract, flashloan_fee)
                .tx
                .data
                .unwrap()
                .0
                .clone(),
            current_safe_nonce + U256::from(2),
        );

        let app_data = format!(
            r#"{{
                "metadata": {{
                    "flashloan": {{
                        "lender": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
                        "borrower": "{:?}",
                        "token": "{:?}",
                        "amount": "{:?}"
                    }},
                    "hooks": {{
                        "pre": [
                            {{
                                "target": "{:?}",
                                "callData": "0x{}",
                                "gasLimit": "1000000"
                            }},
                            {{
                                "target": "{:?}",
                                "callData": "0x{}",
                                "gasLimit": "1000000"
                            }}
                        ],
                        "post": [
                            {{
                                "target": "{:?}",
                                "callData": "0x{}",
                                "gasLimit": "1000000"
                            }}
                        ]
                    }},
                    "signer": "{:?}"
                }}
            }}"#,
            // borrower
            trader.address(),
            // flashloan token
            weth.address(),
            // take out a flash loan to pay back what was borrowed against collateral
            debt_amount,
            // 1st pre-hook to pay back borrowed 1 weth
            trader.address(),
            hex::encode(&repay_debt_tx.tx.data.unwrap().0),
            // 2nd pre-hook to get collateral out
            // ~200K gas
            trader.address(),
            hex::encode(&withdraw_tx.tx.data.unwrap().0),
            // 1st post-hook
            trader.address(),
            hex::encode(&repay_flashloan_fee.tx.data.unwrap().0),
            // signer
            trader.address(),
        );

        OrderCreationAppData::Full {
            full: app_data.to_string(),
        }
    };

    let mut order = OrderCreation {
        sell_token: usdc.address(),
        sell_amount: collateral_amount,
        buy_token: weth.address(),
        // we want to get exactly enough WETH to repay the flashloan w/e fee
        buy_amount: debt_amount + flashloan_fee,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        ..Default::default()
    };
    order.signature = Signature::Eip1271(trader.sign_message(&hashed_eip712_message(
        &onchain.contracts().domain_separator,
        &order.data().hash_struct(),
    )));

    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

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
    // (~4900 USDC for ~1 WETH) they have that much less `USDC` compared to
    // the original collateral.
    let trader_usdc = balance(&web3, trader.address(), usdc.address()).await;
    assert!(trader_usdc > to_wei_with_exp(45_000, 6));
    tracing::info!("trader got majority of collateral back");

    let trader_weth = balance(&web3, trader.address(), weth.address()).await;
    assert_eq!(trader_weth, 0.into());
    tracing::info!("the trader spent all their weth");

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
