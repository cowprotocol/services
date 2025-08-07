use {
    chrono::{NaiveDateTime, Utc},
    contracts::{
        ERC20,
        ILiquoriceSettlement,
        IZeroEx,
        LiquoriceAllowListAuthentication,
        i_zero_ex::Contract,
    },
    driver::domain::{competition::order::Partial::No, eth::H160},
    e2e::{
        api::{
            liquorice::{DomainSeparator, Eip712TypedLiquoriceSingleOrder, LiquoriceSignature},
            zeroex::{Eip712TypedZeroExOrder, ZeroExApi},
        },
        assert_approximately_eq,
        nodes::forked_node::ForkedNodeApi,
        setup::{
            OnchainComponents,
            Services,
            TIMEOUT,
            TestAccount,
            colocation::{self, SolverEngine},
            mock::Mock,
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
        },
        tx,
    },
    ethcontract::{
        Account,
        Bytes,
        H256,
        errors::MethodError,
        prelude::U256,
        transaction::TransactionResult,
    },
    ethrpc::Web3,
    hex_literal::hex,
    model::{
        order::{OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::addr,
    solvers_dto::solution::Solution,
    std::collections::HashMap,
    web3::signing::SecretKeyRef,
};

/// The block number from which we will fetch state for the forked tests.
pub const FORK_BLOCK: u64 = 21839998;
pub const USDT_WHALE: H160 = H160(hex!("F977814e90dA44bFA03b6295A0616a897441aceC"));
pub const USDC_WHALE: H160 = H160(hex!("28c6c06298d514db089934071355e5743bf21d60"));

pub const LIQUORICE_MANAGER: H160 = H160(hex!("000438801500c89E225E8D6CB69D9c14dD05e000"));

#[tokio::test]
#[ignore]
async fn forked_node_liquidity_sources_notification_mainnet() {
    run_forked_test_with_block_number(
        liquidity_sources_notification,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK,
    )
    .await
}

async fn liquidity_sources_notification(web3: Web3) {
    /*
     * Arrange
     */
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    // # Define trade params
    let trade_amount = to_wei_with_exp(5, 8);

    // # Create parties accounts
    //   solver - represents both baseline solver engine for quoting and liquorice
    //   solver engine for solving
    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    // trader - the account that will place the order
    // liquorice_maker - the account that will be used to fill the order
    let [trader, liquorice_maker] = onchain.make_accounts(to_wei(1)).await;

    // # Access trade tokens contracts
    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_usdt = ERC20::at(
        &web3,
        "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .unwrap(),
    );

    // # CoW onchain setup
    {
        // Fund trader
        let usdc_whale = forked_node_api.impersonate(&USDC_WHALE).await.unwrap();
        tx!(
            usdc_whale,
            token_usdc.transfer(trader.address(), trade_amount)
        );

        // Fund solver
        // TODO: remove?
        // Fund solver
        tx!(
            usdc_whale,
            token_usdc.transfer(solver.address(), trade_amount)
        );

        // Trader gives approval to the CoW allowance contract
        tx!(
            trader.account(),
            token_usdc.approve(onchain.contracts().allowance, trade_amount)
        );
    }

    // # Liquorice onchain setup
    // Liquorice Settlement contract through which we will trade with the
    // `liquorice_maker`
    let liquorice_settlement = ILiquoriceSettlement::deployed(&web3).await.unwrap();

    // Fund `liquorice_maker`
    {
        let usdt_whale = forked_node_api.impersonate(&USDT_WHALE).await.unwrap();
        tx!(
            usdt_whale,
            token_usdt.transfer(liquorice_maker.address(), trade_amount)
        );
    }

    // Maker gives approval to Liquorice Balance manager contract
    tx!(
        liquorice_maker.account(),
        token_usdt.approve(
            liquorice_settlement
                .balance_manager()
                .call()
                .await
                .expect("balance manager"),
            trade_amount
        )
    );

    // Liquorice manager whitelists maker and CoW settlement contract
    {
        let liquorice_allowlist = LiquoriceAllowListAuthentication::at(
            &web3,
            liquorice_settlement.authenticator().call().await.unwrap(),
        );

        let liquorice_manager = forked_node_api
            .impersonate(&LIQUORICE_MANAGER)
            .await
            .unwrap();

        // Add maker to the allowlist of makers
        tx!(
            liquorice_manager,
            liquorice_allowlist.add_maker(liquorice_maker.address())
        );

        // Add GPV2Settlement to the allowlist of solvers
        tx!(
            liquorice_manager,
            liquorice_allowlist.add_solver(onchain.contracts().gp_settlement.address())
        );
    }

    // # CoW services setup
    let liquorice_solver_api_mock = Mock::default();
    let services = Services::new(&onchain).await;

    let base_tokens = vec![token_usdc.address(), token_usdt.address()];
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
                name: "liquorice_solver".into(),
                account: solver.clone(),
                endpoint: liquorice_solver_api_mock.url.clone(),
                base_tokens: vec![],
                merge_solutions: true,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    services
        .start_autopilot(
            None,
            vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                format!(
                    "--drivers=liquorice_solver|http://localhost:11088/liquorice_solver|{}",
                    hex::encode(solver.address())
                ),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    /*
     * Act
     */

    // # Mint block
    onchain.mint_block().await;

    // # Create CoW order
    let order_id = {
        let order = OrderCreation {
            sell_token: token_usdc.address(),
            sell_amount: trade_amount,
            buy_token: token_usdt.address(),
            buy_amount: trade_amount,
            valid_to: model::time::now_in_epoch_seconds() + 300,
            kind: OrderKind::Sell,
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        );
        services.create_order(&order).await.unwrap()
    };

    // # Create Liquorice solution calldata for CoW order
    let liquorice_solution_calldata = {
        // Create Liquorice order
        let now = Utc::now().timestamp() as u64;
        let liquorice_order = Eip712TypedLiquoriceSingleOrder {
            rfq_id: "c99d2e3f-702b-49c9-8bb8-43775770f2f3".to_string(),
            nonce: U256::from(0),
            trader: onchain.contracts().gp_settlement.address(),
            effective_trader: onchain.contracts().gp_settlement.address(),
            base_token: token_usdc.address(),
            quote_token: token_usdt.address(),
            base_token_amount: trade_amount,
            quote_token_amount: trade_amount,
            min_fill_amount: U256::from(1),
            quote_expiry: U256::from(Utc::now().timestamp() as u64 + 10),
            recipient: liquorice_maker.address(),
        };

        // Create Liquorice order signature
        let liquorice_order_signature = liquorice_order.sign(
            &DomainSeparator::new(1, liquorice_settlement.address()),
            liquorice_order.hash_struct(),
            &liquorice_maker,
        );

        // Create Liquorice settlement calldata
        liquorice_settlement
            .settle_single(
                liquorice_maker.address().into(),
                liquorice_order.as_tuple(),
                liquorice_order_signature.as_tuple(),
                liquorice_order.quote_token_amount,
                // Taker signature is not used in this use case
                LiquoriceSignature {
                    signature_type: 0,
                    transfer_command: 0,
                    signature_bytes: ethcontract::Bytes(vec![0u8; 65]),
                }
                .as_tuple(),
            )
            .tx
            .data
            .unwrap()
    };

    // # Submit solution to the CoW
    liquorice_solver_api_mock.configure_solution(Some(Solution {
        id: 1,
        prices: HashMap::from([
            (token_usdc.address(), to_wei(1)),
            (token_usdt.address(), to_wei(1)),
        ]),
        trades: vec![solvers_dto::solution::Trade::Fulfillment(
            solvers_dto::solution::Fulfillment {
                executed_amount: trade_amount,
                fee: Some(0.into()),
                order: solvers_dto::solution::OrderUid(order_id.0),
            },
        )],
        pre_interactions: vec![],
        interactions: vec![solvers_dto::solution::Interaction::Custom(
            solvers_dto::solution::CustomInteraction {
                target: liquorice_settlement.address(),
                calldata: liquorice_solution_calldata.0,
                value: 0.into(),
                allowances: vec![],
                inputs: vec![],
                outputs: vec![],
                internalize: false,
            },
        )],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
    }));

    /*
     * Assert
     */

    tracing::info!("Waiting for trade to get indexed");
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let trade = services.get_trades(&order_id).await.unwrap().pop()?;
        Some(
            services
                .get_solver_competition(trade.tx_hash?)
                .await
                .is_ok(),
        )
    })
    .await
    .unwrap();

    let trade = services.get_trades(&order_id).await.unwrap().pop().unwrap();
}

fn create_zeroex_liquidity_orders(
    order_creation: OrderCreation,
    zeroex_maker: TestAccount,
    zeroex_addr: H160,
    chain_id: u64,
    weth_address: H160,
) -> [shared::zeroex_api::OrderRecord; 3] {
    let typed_order = Eip712TypedZeroExOrder {
        maker_token: order_creation.buy_token,
        taker_token: order_creation.sell_token,
        // fully covers execution costs
        maker_amount: order_creation.buy_amount.as_u128() * 3,
        taker_amount: order_creation.sell_amount.as_u128() * 2,
        // makes 0x order partially filled, but the amount is higher than the cowswap order to
        // make sure the 0x order is not overfilled in the end of the e2e test
        remaining_fillable_taker_amount: order_creation.sell_amount.as_u128() * 3 / 2,
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        // Makes it possible for anyone to fill the order
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    let usdt_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.buy_token,
        // the value comes from the `--amount-to-estimate-prices-with` config to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.as_u128(),
        remaining_fillable_taker_amount: order_creation.sell_amount.as_u128(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
    };
    let usdc_weth_order = Eip712TypedZeroExOrder {
        maker_token: weth_address,
        taker_token: order_creation.sell_token,
        // the value comes from the `--amount-to-estimate-prices-with` config to provide
        // sufficient liquidity
        maker_amount: 1_000_000_000_000_000_000u128,
        taker_amount: order_creation.sell_amount.as_u128(),
        remaining_fillable_taker_amount: order_creation.sell_amount.as_u128(),
        taker_token_fee_amount: 0,
        maker: zeroex_maker.address(),
        taker: Default::default(),
        sender: Default::default(),
        fee_recipient: zeroex_addr,
        pool: H256::default(),
        expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
        salt: U256::from(Utc::now().timestamp()),
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
    zeroex: &Contract,
    zeroex_order: &shared::zeroex_api::OrderRecord,
) -> Result<ZeroExOrderAmounts, MethodError> {
    zeroex
        .get_limit_order_relevant_state(
            (
                zeroex_order.order().maker_token,
                zeroex_order.order().taker_token,
                zeroex_order.order().maker_amount,
                zeroex_order.order().taker_amount,
                zeroex_order.order().taker_token_fee_amount,
                zeroex_order.order().maker,
                zeroex_order.order().taker,
                zeroex_order.order().sender,
                zeroex_order.order().fee_recipient,
                Bytes(zeroex_order.order().pool.0),
                zeroex_order.order().expiry,
                zeroex_order.order().salt,
            ),
            (
                zeroex_order.order().signature.signature_type,
                zeroex_order.order().signature.v,
                Bytes(zeroex_order.order().signature.r.0),
                Bytes(zeroex_order.order().signature.s.0),
            ),
        )
        .call()
        .await
        .map(|((_, _, filled), fillable, _)| ZeroExOrderAmounts { filled, fillable })
}

async fn fill_or_kill_zeroex_limit_order(
    zeroex: &Contract,
    zeroex_order: &shared::zeroex_api::OrderRecord,
    from_account: Account,
) -> Result<TransactionResult, MethodError> {
    zeroex
        .fill_or_kill_limit_order(
            (
                zeroex_order.order().maker_token,
                zeroex_order.order().taker_token,
                zeroex_order.order().maker_amount,
                zeroex_order.order().taker_amount,
                zeroex_order.order().taker_token_fee_amount,
                zeroex_order.order().maker,
                zeroex_order.order().taker,
                zeroex_order.order().sender,
                zeroex_order.order().fee_recipient,
                Bytes(zeroex_order.order().pool.0),
                zeroex_order.order().expiry,
                zeroex_order.order().salt,
            ),
            (
                zeroex_order.order().signature.signature_type,
                zeroex_order.order().signature.v,
                Bytes(zeroex_order.order().signature.r.0),
                Bytes(zeroex_order.order().signature.s.0),
            ),
            zeroex_order.order().taker_amount,
        )
        .from(from_account)
        .send()
        .await
}
