use {
    crate::ethflow::ExtendedEthFlowOrder,
    ::alloy::{primitives::Address, providers::ext::AnvilApi},
    chrono::Utc,
    contracts::alloy::{CoWSwapEthFlow, ERC20Mintable},
    e2e::setup::*,
    ethrpc::{Web3, alloy::EvmProviderExt, block_stream::timestamp_of_current_block_in_seconds},
    model::{
        order::OrderUid,
        quote::{OrderQuoteRequest, OrderQuoteSide, QuoteSigningScheme, Validity},
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    refunder::{RefundStatus, refund_service::RefundService},
    rstest::{Context, rstest},
};

// Common constants for refunder tests
const SELL_AMOUNT: u128 = 3_000_000_000_000_000;
const MAX_GAS_PRICE: u64 = 2_000_000_000_000; // 2000 Gwei
const START_PRIORITY_FEE_TIP: u64 = 30_000_000_000; // 30 Gwei

/// Advances the blockchain time past the given expiration timestamp.
async fn advance_time_past_expiration(web3: &Web3, valid_to: u32) {
    // Add 60 seconds buffer so the order is definitively expired, not just at the boundary.
    let target_timestamp = valid_to as u64 + 60;
    web3.alloy
        .evm_set_next_block_timestamp(target_timestamp)
        .await
        .expect("Must be able to set block timestamp");
    web3.alloy
        .evm_mine(None)
        .await
        .expect("Unable to mine next block");
}

/// Waits for an order to be indexed by the orderbook service.
async fn wait_for_order_indexed(
    services: &Services<'_>,
    onchain: &OnchainComponents,
    order_id: &OrderUid,
) {
    tracing::info!("Waiting for order to be indexed.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services.get_order(order_id).await.is_ok()
    })
    .await
    .expect("Timed out waiting for order to be indexed");
}

/// Creates a standard quote request for ethflow orders.
fn default_quote_request(
    from: Address,
    weth: &contracts::alloy::WETH9::Instance,
    buy_token: Address,
    sell_amount: NonZeroU256,
) -> OrderQuoteRequest {
    OrderQuoteRequest {
        from,
        sell_token: *weth.address(),
        buy_token,
        receiver: Some(Address::repeat_byte(42)),
        validity: Validity::For(3600),
        signing_scheme: QuoteSigningScheme::Eip1271 {
            onchain_order: true,
            verification_gas_limit: 0,
        },
        side: OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::AfterFee { value: sell_amount },
        },
        ..Default::default()
    }
}

/// Common test infrastructure for refunder tests.
///
/// This setup is optimized for testing **refund eligibility**, not settlement.
/// Orders created through this setup typically use high slippage (9999 bps),
/// which makes them eligible for refund (the refunder's SQL query filters by
/// minimum price deviation).
///
/// This setup uses blockchain time for `valid_to` timestamps, which works
/// correctly for refund-eligibility tests (the refunder also uses blockchain
/// time). However, tests requiring actual **settlement** must use real-world
/// timestamps (`Utc::now()`) because autopilot validates against wall-clock
/// time. See `refunder_skips_settled_orders` for an example.
///
/// Note: Both `onchain` and `services` are leaked (via `Box::leak`) to avoid
/// lifetime issues. This should be safe enough in test code.
/// Alternatives to leaking inlcude:
/// - Transmuting, but requires unsafe and introduces some drop complexity
/// - Borrow services & onchain structs
/// - Use Rc/Arc
struct RefunderTestSetup {
    web3: Web3,
    services: &'static Services<'static>,
    onchain: &'static OnchainComponents,
    user: TestAccount,
    refunder_account: TestAccount,
    buy_token: Address,
}

impl RefunderTestSetup {
    async fn new(web3: Web3) -> Self {
        let mut onchain = OnchainComponents::deploy(web3.clone()).await;

        let [solver] = onchain.make_solvers(10u64.eth()).await;
        let [user, refunder_account] = onchain.make_accounts(10u64.eth()).await;
        let [token] = onchain
            .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
            .await;
        let buy_token = *token.address();

        // Leak onchain first to get a 'static reference
        let onchain: &'static OnchainComponents = Box::leak(Box::new(onchain));

        let services = Services::new(onchain).await;
        services.start_protocol(solver).await;

        // Then leak services to obtain a 'static lifetime.
        let services: &'static Services<'static> = Box::leak(Box::new(services));

        Self {
            web3,
            services,
            onchain,
            user,
            refunder_account,
            buy_token,
        }
    }

    fn ethflow_contract(&self) -> &CoWSwapEthFlow::Instance {
        self.onchain.contracts().ethflows.first().unwrap()
    }

    fn buy_token(&self) -> Address {
        self.buy_token
    }

    async fn create_and_index_ethflow_order(
        &self,
        slippage_bps: u16,
        validity_duration: u32,
    ) -> (ExtendedEthFlowOrder, OrderUid, u32) {
        let sell_amount = NonZeroU256::try_from(SELL_AMOUNT).unwrap();
        let ethflow_contract = self.ethflow_contract();

        let quote = default_quote_request(
            *ethflow_contract.address(),
            &self.onchain.contracts().weth,
            self.buy_token(),
            sell_amount,
        );
        let quote_response = self.services.submit_quote(&quote).await.unwrap();

        let valid_to = timestamp_of_current_block_in_seconds(&self.web3.alloy)
            .await
            .unwrap()
            + validity_duration;

        let ethflow_order = ExtendedEthFlowOrder::from_quote(&quote_response, valid_to)
            .include_slippage_bps(slippage_bps);

        ethflow_order
            .mine_order_creation(self.user.address(), ethflow_contract)
            .await;

        let order_id = ethflow_order
            .uid(self.onchain.contracts(), ethflow_contract)
            .await;

        wait_for_order_indexed(self.services, self.onchain, &order_id).await;

        (ethflow_order, order_id, valid_to)
    }

    fn create_refund_service(
        &self,
        ethflow_contracts: Vec<CoWSwapEthFlow::Instance>,
        min_validity_duration: i64,
        min_price_deviation_bps: i64,
        signer: ::alloy::signers::local::PrivateKeySigner,
    ) -> RefundService {
        create_refund_service(
            self.services,
            self.web3.clone(),
            ethflow_contracts,
            min_validity_duration,
            min_price_deviation_bps,
            signer,
        )
    }
}

fn create_refund_service(
    services: &Services<'_>,
    web3: Web3,
    ethflow_contracts: Vec<CoWSwapEthFlow::Instance>,
    min_validity_duration: i64,
    min_price_deviation_bps: i64,
    signer: ::alloy::signers::local::PrivateKeySigner,
) -> RefundService {
    RefundService::new(
        services.db().clone(),
        web3,
        ethflow_contracts,
        min_validity_duration,
        min_price_deviation_bps,
        Box::new(signer),
        MAX_GAS_PRICE,
        START_PRIORITY_FEE_TIP,
    )
}

/// Pair of order's validity duration and refunder's enforced minimum.
#[derive(Debug, Clone, Copy)]
struct ValidityDuration {
    /// The order's validity duration (valid_to - creation_timestamp).
    order: u32,
    /// The refunder's minimum validity duration threshold.
    enforced: i64,
}

/// Pair of order's slippage and refunder's enforced minimum price deviation.
#[derive(Debug, Clone, Copy)]
struct SlippageBps {
    /// The order's slippage in basis points.
    order: u16,
    /// The refunder's minimum price deviation threshold in basis points.
    enforced: i64,
}

#[tracing::instrument]
async fn run_refunder_threshold_test(
    web3: Web3,
    slippage: SlippageBps,
    validity: ValidityDuration,
    expect_refund: bool,
    description: &str,
) {
    tracing::info!("Running refunder threshold test");

    // Settlement is intentionally impossible in this test due to timestamp mismatch:
    //
    // 1. Anvil starts at Jan 1, 2020 (see `--timestamp 1577836800` in nodes/mod.rs)
    // 2. This test uses blockchain time for `valid_to` via `timestamp_of_current_block_in_seconds()`
    // 3. The autopilot filters orders using wall-clock time (`SystemTime::now()`)
    // 4. Since blockchain time (~2020) << wall-clock time (~2026), orders appear
    //    immediately expired to the autopilot and are never included in auctions
    //
    // This is intentional: refunder threshold tests only verify refund eligibility,
    // not settlement. The refunder itself uses blockchain time, so these "expired"
    // orders are correctly processed for refund. Tests that need actual settlement
    // (e.g., `refunder_skips_settled_orders`) use `Utc::now()` for `valid_to` instead.
    let setup = RefunderTestSetup::new(web3.clone()).await;
    let ethflow_contract = setup.ethflow_contract();

    let (ethflow_order, _order_id, valid_to) = setup
        .create_and_index_ethflow_order(slippage.order, validity.order)
        .await;

    advance_time_past_expiration(&web3, valid_to).await;

    let mut refund_service = setup.create_refund_service(
        vec![ethflow_contract.clone()],
        validity.enforced,
        slippage.enforced,
        setup.refunder_account.signer.clone(),
    );

    // Verify order is not yet invalidated
    assert_ne!(
        ethflow_order
            .status(setup.onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded
    );

    refund_service
        .try_to_refund_all_eligible_orders()
        .await
        .unwrap();

    // Check the expected outcome
    let status = ethflow_order
        .status(setup.onchain.contracts(), ethflow_contract)
        .await;

    assert!(
        expect_refund == (status == RefundStatus::Refunded),
        "Test failed: {description}.\nExpected refund: {expect_refund}, but got status: {status:?}"
    );
}

#[rstest]
// Tests that orders with slippage below, at, or above the min_price_deviation
// threshold are refunded according to the SQL >= check.
#[case::slippage_below_threshold(
    SlippageBps { order: 50, enforced: 100 },
    ValidityDuration { order: 600, enforced: 0 },
    false
)]
#[case::slippage_above_threshold(
    SlippageBps { order: 500, enforced: 100 },
    ValidityDuration { order: 600, enforced: 0 },
    true
)]
#[case::slippage_at_boundary(
    SlippageBps { order: 100, enforced: 100 },
    ValidityDuration { order: 600, enforced: 0 },
    true
)]
// Tests that orders with validity duration below, at, or above the
// min_validity_duration threshold are refunded according to the SQL > check.
#[case::validity_below_duration(
    SlippageBps { order: 9999, enforced: 0 },
    ValidityDuration { order: 100, enforced: 200 },
    false
)]
#[case::validity_above_duration(
    SlippageBps { order: 9999, enforced: 0 },
    ValidityDuration { order: 600, enforced: 100 },
    true
)]
#[case::validity_at_duration_boundary(
    SlippageBps { order: 9999, enforced: 0 },
    ValidityDuration { order: 100, enforced: 100 },
    false
)]
#[ignore]
#[tokio::test]
async fn local_node_refunder_thresholds(
    #[case] slippage: SlippageBps,
    #[case] validity: ValidityDuration,
    #[case] expect_refund: bool,
    #[context] context: Context,
) {
    let description = context.description.unwrap_or("unknown");
    run_test(|web3| {
        run_refunder_threshold_test(web3, slippage, validity, expect_refund, description)
    })
    .await;
}

/// Test that orders already invalidated on-chain by the user are NOT refunded
/// by the refunder service (SQL filters: o_inv.uid is null).
#[ignore]
#[tokio::test]
async fn local_node_refunder_skips_invalidated_orders() {
    run_test(refunder_skips_invalidated_orders).await;
}

async fn refunder_skips_invalidated_orders(web3: Web3) {
    tracing::info!("Testing that already-invalidated orders are skipped by the refunder");

    let setup = RefunderTestSetup::new(web3.clone()).await;
    let ethflow_contract = setup.ethflow_contract();

    // Use high slippage (9999 bps) so the order would normally be eligible for
    // refund
    let (ethflow_order, order_id, valid_to) = setup.create_and_index_ethflow_order(9999, 600).await;

    // User invalidates the order on-chain BEFORE the refunder runs
    tracing::info!("User invalidating order on-chain.");
    ethflow_order
        .mine_order_invalidation(setup.user.address(), ethflow_contract)
        .await;

    // Wait for the invalidation to be indexed by the autopilot
    tracing::info!("Waiting for invalidation to be indexed.");
    wait_for_condition(TIMEOUT, || async {
        setup.onchain.mint_block().await;
        let order = setup.services.get_order(&order_id).await.unwrap();
        order.metadata.status == model::order::OrderStatus::Cancelled
    })
    .await
    .unwrap();

    advance_time_past_expiration(&web3, valid_to).await;

    let mut refund_service = setup.create_refund_service(
        vec![ethflow_contract.clone()],
        0, // min_validity_duration = 0 (permissive)
        0, // min_price_deviation_bps = 0 (permissive)
        setup.refunder_account.signer.clone(),
    );

    // The order should already be invalidated on-chain before the refunder runs
    assert_eq!(
        ethflow_order
            .status(setup.onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded,
        "Order should already be invalidated by user"
    );

    // Run the refunder - it should NOT try to refund this already-invalidated order
    refund_service
        .try_to_refund_all_eligible_orders()
        .await
        .unwrap();

    // The order should still be invalidated (status unchanged)
    assert_eq!(
        ethflow_order
            .status(setup.onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded
    );

    // Verify no refund TX was recorded (the refunder didn't process this order)
    let order = setup.services.get_order(&order_id).await.unwrap();
    assert!(
        order
            .metadata
            .ethflow_data
            .unwrap()
            .refund_tx_hash
            .is_none(),
        "Refunder should not have created a refund TX for an already-invalidated order"
    );
}

/// Test that orders already settled (with trades) are NOT refunded
/// by the refunder service (SQL filters: t.order_uid is null).
///
/// This test doesn't use RefunderTestSetup because it needs actual settlement,
/// which requires real-world timestamps (autopilot validates against wall-clock
/// time).
#[ignore]
#[tokio::test]
async fn local_node_refunder_skips_settled_orders() {
    run_test(refunder_skips_settled_orders).await;
}

async fn refunder_skips_settled_orders(web3: Web3) {
    tracing::info!("Testing that already-settled orders are skipped by the refunder");

    // This test requires special setup for the order to be fillable by solvers:
    // larger sell amount (1 ETH), low slippage (300 bps), and real-world
    // timestamps for autopilot validation.
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [user, refunder_account] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let buy_token = *token.address();
    let receiver = Address::repeat_byte(42);
    let sell_amount = NonZeroU256::try_from(1u64.eth()).unwrap();

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    let quote = default_quote_request(
        *ethflow_contract.address(),
        &onchain.contracts().weth,
        buy_token,
        sell_amount,
    );
    let quote_response = services.submit_quote(&quote).await.unwrap();

    // Anvil starts with a hardcoded timestamp of Jan 1, 2020 (see nodes/mod.rs).
    // The autopilot validates orders against real-world time, so valid_to must
    // exceed the current wall clock (which also exceeds anvil's simulated time).
    let valid_to = Utc::now().timestamp() as u32 + 3600;

    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(300);

    ethflow_order
        .mine_order_creation(user.address(), ethflow_contract)
        .await;

    let order_id = ethflow_order
        .uid(onchain.contracts(), ethflow_contract)
        .await;

    wait_for_order_indexed(&services, &onchain, &order_id).await;

    // Wait for the order to be settled (receiver gets buy tokens)
    tracing::info!("Waiting for order to be settled.");
    let buy_token_contract = ERC20Mintable::Instance::new(buy_token, web3.alloy.clone());
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let balance = buy_token_contract
            .balanceOf(receiver)
            .call()
            .await
            .expect("Unable to get token balance");
        balance >= ethflow_order.0.buyAmount
    })
    .await
    .unwrap();

    tracing::info!("Order was settled. Now advancing time past expiration.");
    advance_time_past_expiration(&web3, valid_to).await;

    // Verify the order is NOT invalidated on-chain (it was settled, not
    // invalidated)
    assert_ne!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded,
        "Settled order should not be invalidated on-chain"
    );

    let mut refund_service = create_refund_service(
        &services,
        web3.clone(),
        vec![ethflow_contract.clone()],
        0, // min_validity_duration = 0 (permissive)
        0, // min_price_deviation_bps = 0 (permissive)
        refunder_account.signer,
    );

    // Run the refunder - it should NOT try to refund this already-settled order
    refund_service
        .try_to_refund_all_eligible_orders()
        .await
        .unwrap();

    // The order should still NOT be invalidated (refunder skipped it)
    assert_ne!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded,
        "Refunder should not have invalidated the already-settled order"
    );

    // Verify no refund TX was recorded
    let order = services.get_order(&order_id).await.unwrap();
    assert!(
        order
            .metadata
            .ethflow_data
            .unwrap()
            .refund_tx_hash
            .is_none(),
        "Refunder should not have created a refund TX for an already-settled order"
    );
}

#[ignore]
#[tokio::test]
async fn local_node_refunder_tx() {
    run_test(refunder_tx).await;
}

async fn refunder_tx(web3: Web3) {
    // This test creates orders on TWO different ethflow contracts
    // to verify the refunder can handle multiple contracts.
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [user, refunder] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let buy_token = *token.address();
    let sell_amount = NonZeroU256::try_from(SELL_AMOUNT).unwrap();

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    let ethflow_contract_2 = onchain.contracts().ethflows.get(1).unwrap();

    // Create first order on primary ethflow contract
    let quote = default_quote_request(
        *ethflow_contract.address(),
        &onchain.contracts().weth,
        buy_token,
        sell_amount,
    );
    let quote_response = services.submit_quote(&quote).await.unwrap();

    let validity_duration = 600u32;
    let valid_to = timestamp_of_current_block_in_seconds(&web3.alloy)
        .await
        .unwrap()
        + validity_duration;

    // High slippage (9999 bps) for the order to be picked up by the refunder
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(9999);

    // Create second order on secondary ethflow contract
    let quote_2 = default_quote_request(
        *ethflow_contract_2.address(),
        &onchain.contracts().weth,
        buy_token,
        sell_amount,
    );
    let quote_response_2 = services.submit_quote(&quote_2).await.unwrap();
    let ethflow_order_2 =
        ExtendedEthFlowOrder::from_quote(&quote_response_2, valid_to).include_slippage_bps(9999);

    // Mine both orders
    ethflow_order
        .mine_order_creation(user.address(), ethflow_contract)
        .await;
    ethflow_order_2
        .mine_order_creation(user.address(), ethflow_contract_2)
        .await;

    let order_id = ethflow_order
        .uid(onchain.contracts(), ethflow_contract)
        .await;
    let order_id_2 = ethflow_order_2
        .uid(onchain.contracts(), ethflow_contract_2)
        .await;

    // Wait for both orders to be indexed
    tracing::info!("Waiting for orders to be indexed.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services.get_order(&order_id).await.is_ok() && services.get_order(&order_id_2).await.is_ok()
    })
    .await
    .unwrap();

    advance_time_past_expiration(&web3, valid_to).await;

    let mut refund_service = create_refund_service(
        &services,
        web3,
        vec![ethflow_contract.clone(), ethflow_contract_2.clone()],
        validity_duration as i64 / 2,
        10,
        refunder.signer,
    );

    // Verify orders are not yet refunded
    assert_ne!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded
    );
    assert_ne!(
        ethflow_order_2
            .status(onchain.contracts(), ethflow_contract_2)
            .await,
        RefundStatus::Refunded
    );

    refund_service
        .try_to_refund_all_eligible_orders()
        .await
        .unwrap();

    // Both orders should now be refunded
    assert_eq!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded
    );
    assert_eq!(
        ethflow_order_2
            .status(onchain.contracts(), ethflow_contract_2)
            .await,
        RefundStatus::Refunded
    );

    // Wait for autopilot to index refund tx hashes
    tracing::info!("Waiting for autopilot to index refund tx hash.");
    for order in &[order_id, order_id_2] {
        let has_tx_hash = || async {
            onchain.mint_block().await;
            services
                .get_order(order)
                .await
                .unwrap()
                .metadata
                .ethflow_data
                .unwrap()
                .refund_tx_hash
                .is_some()
        };
        wait_for_condition(TIMEOUT, has_tx_hash).await.unwrap();
    }
}
