//! # Regarding Order Settlement
//!
//! For tests requiring actual settlement, use `Utc::now()` for `valid_to` when
//! creating a new test order. See `refunder_skips_settled_orders` for an
//! example.

use {
    crate::ethflow::ExtendedEthFlowOrder,
    ::alloy::{primitives::Address, providers::ext::AnvilApi},
    chrono::Utc,
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
const SELL_AMOUNT: u128 = 3_000_000_000_000_000; // 0.003 ETH
const MAX_GAS_PRICE: u64 = 2_000_000_000_000; // 2000 Gwei
const START_PRIORITY_FEE_TIP: u64 = 30_000_000_000; // 30 Gwei
const SLIPPAGE_BPS: u16 = 300; // 3%
const RECEIVER: Address = Address::repeat_byte(42);

/// Advances the blockchain time past the given expiration timestamp.
async fn advance_time_past_expiration(web3: &Web3, valid_to: u32) {
    // Add 60 seconds buffer so the order is definitively expired, not just at the
    // boundary.
    let target_timestamp = valid_to as u64 + 60;
    web3.provider
        .evm_set_next_block_timestamp(target_timestamp)
        .await
        .expect("Must be able to set block timestamp");
    web3.provider
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

/// Waits for an order to be settled and indexed in the database.
async fn wait_for_order_settlement(
    services: &Services<'_>,
    onchain: &OnchainComponents,
    order_id: &OrderUid,
) {
    tracing::info!("Waiting for order to be settled.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(order_id)
            .await
            .map(|o| o.metadata.status == model::order::OrderStatus::Fulfilled)
            .unwrap_or(false)
    })
    .await
    .expect("Timed out waiting for order to be settled");
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
        receiver: Some(RECEIVER),
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

/// Builder for creating and indexing ethflow orders in tests.
struct EthflowOrderBuilder<'a> {
    // Core dependencies
    services: &'a Services<'a>,
    onchain: &'a OnchainComponents,
    user: &'a TestAccount,
    buy_token: Address,

    // Builder state (SQL filter control)
    sell_amount: NonZeroU256,
    slippage_bps: u16,
    valid_to: u32,
    should_invalidate: bool,
    ethflow_index: usize,
}

impl<'a> EthflowOrderBuilder<'a> {
    /// Create a new builder with sensible defaults
    fn new(
        services: &'a Services<'a>,
        onchain: &'a OnchainComponents,
        user: &'a TestAccount,
        buy_token: Address,
    ) -> Self {
        Self {
            services,
            onchain,
            user,
            buy_token,
            sell_amount: NonZeroU256::try_from(SELL_AMOUNT).unwrap(),
            slippage_bps: SLIPPAGE_BPS,
            valid_to: 60,
            should_invalidate: false,
            ethflow_index: 0,
        }
    }

    /// Set the sell amount for the order.
    fn with_sell_amount(mut self, amount: NonZeroU256) -> Self {
        self.sell_amount = amount;
        self
    }

    /// Set slippage in basis points (e.g., 500 = 5%).
    fn with_slippage_bps(mut self, bps: u16) -> Self {
        self.slippage_bps = bps;
        self
    }

    /// Set order expiration timestamp (absolute, seconds since epoch).
    fn with_valid_to(mut self, valid_to: u32) -> Self {
        self.valid_to = valid_to;
        self
    }

    /// Mark order as invalidated on-chain after creation.
    fn invalidated(mut self) -> Self {
        self.should_invalidate = true;
        self
    }

    /// Select which ethflow contract to use.
    fn with_ethflow_index(mut self, index: usize) -> Self {
        self.ethflow_index = index;
        self
    }

    /// Creates the order, mines it on-chain, waits for indexing, and optionally
    /// invalidates.
    async fn create_and_index(self) -> (ExtendedEthFlowOrder, OrderUid, u32) {
        let ethflow_contract = self
            .onchain
            .contracts()
            .ethflows
            .get(self.ethflow_index)
            .expect("could not locate ethflow contract at given position");

        // Get quote
        let quote = default_quote_request(
            *ethflow_contract.address(),
            &self.onchain.contracts().weth,
            self.buy_token,
            self.sell_amount,
        );
        let quote_response = self.services.submit_quote(&quote).await.unwrap();

        let valid_to = self.valid_to;

        // Create ethflow order with slippage
        let ethflow_order = ExtendedEthFlowOrder::from_quote(&quote_response, valid_to)
            .include_slippage_bps(self.slippage_bps);

        // Mine order creation
        ethflow_order
            .mine_order_creation(self.user.address(), ethflow_contract)
            .await;

        // Get order UID
        let order_id = ethflow_order
            .uid(self.onchain.contracts(), ethflow_contract)
            .await;

        // Wait for indexing
        wait_for_order_indexed(self.services, self.onchain, &order_id).await;

        // Optionally invalidate
        if self.should_invalidate {
            ethflow_order
                .mine_order_invalidation(self.user.address(), ethflow_contract)
                .await;

            // Wait for invalidation to be indexed
            wait_for_condition(TIMEOUT, || async {
                self.onchain.mint_block().await;
                let order = self.services.get_order(&order_id).await.unwrap();
                order.metadata.status == model::order::OrderStatus::Cancelled
            })
            .await
            .unwrap();
        }

        (ethflow_order, order_id, valid_to)
    }
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

/// Runs a refunder threshold test.
///
/// # Settlement Isolation
///
/// Threshold tests verify the refunder's SQL eligibility filters (slippage and
/// validity duration), not settlement behavior. Orders in these tests use
/// blockchain time for `valid_to`, which combined with Anvil's genesis
/// timestamp of Jan 1, 2020 causes the autopilot to reject them as expired (it
/// validates against wall-clock time). This isolation is intentional: the
/// refunder uses blockchain time internally, so it correctly processes these
/// "expired" orders.
#[tracing::instrument]
async fn run_refunder_threshold_test(
    web3: Web3,
    slippage: SlippageBps,
    validity: ValidityDuration,
    expect_refund: bool,
    description: &str,
) {
    tracing::info!("Running refunder threshold test");

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [user, refunder_account] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let buy_token = *token.address();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();

    // Compute absolute valid_to timestamp from blockchain time + duration
    let valid_to = timestamp_of_current_block_in_seconds(&web3.provider)
        .await
        .unwrap()
        + validity.order;

    // Testing slippage/validity boundaries: order slippage >= enforced threshold
    let (ethflow_order, _order_id, valid_to) =
        EthflowOrderBuilder::new(&services, &onchain, &user, buy_token)
            .with_slippage_bps(slippage.order)
            .with_valid_to(valid_to)
            .create_and_index()
            .await;

    advance_time_past_expiration(&web3, valid_to).await;

    let mut refund_service = RefundService::from_components(
        services.db().clone(),
        web3.clone(),
        vec![*ethflow_contract.address()],
        validity.enforced,
        slippage.enforced,
        refunder_account.signer.clone(),
        MAX_GAS_PRICE,
        START_PRIORITY_FEE_TIP,
        None,
    );

    // Verify order is still eligible for refund (not yet reimbursed)
    assert_ne!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded
    );

    refund_service
        .try_to_refund_all_eligible_orders()
        .await
        .unwrap();

    // Check the expected outcome
    let status = ethflow_order
        .status(onchain.contracts(), ethflow_contract)
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
/// by the refunder service (SQL filter: `o_inv.uid is null`).
///
/// Uses wall-clock time for `valid_to` so orders could potentially settle.
/// The on-chain invalidation is the sole reason the refunder skips this order.
#[ignore]
#[tokio::test]
async fn local_node_refunder_skips_invalidated_orders() {
    run_test(refunder_skips_invalidated_orders).await;
}

async fn refunder_skips_invalidated_orders(web3: Web3) {
    tracing::info!("Testing that already-invalidated orders are skipped by the refunder");

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [user, refunder_account] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let buy_token = *token.address();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();

    // Use wall-clock time so orders could potentially settle. The +600s (10 min)
    // validity window allows time for indexing/invalidation before we advance
    // blockchain time past expiration. The invalidation (not expiration) is the
    // sole reason the refunder skips this order.
    let valid_to = Utc::now().timestamp() as u32 + 600;

    // Create an invalidated order. Even though it would pass slippage/validity
    // checks, the SQL filter `o_inv.uid is null` (ethflow_orders.rs:137)
    // excludes it from the refundable set. The refunder uses permissive
    // thresholds (min_validity_duration=0, min_price_deviation_bps=0 at lines
    // 577-582), so slippage and validity are irrelevant.
    let (ethflow_order, order_id, valid_to) = EthflowOrderBuilder::new(
        &services,
        &onchain,
        &user,
        buy_token,
    )
    .with_valid_to(valid_to)
    .invalidated() // KEY: This is what the test verifies
    .create_and_index()
    .await;

    advance_time_past_expiration(&web3, valid_to).await;

    let mut refund_service = RefundService::from_components(
        services.db().clone(),
        web3.clone(),
        vec![*ethflow_contract.address()],
        0, // min_validity_duration = 0 (permissive)
        0, // min_price_deviation_bps = 0 (permissive)
        refunder_account.signer.clone(),
        MAX_GAS_PRICE,
        START_PRIORITY_FEE_TIP,
        None,
    );

    // The order should already be invalidated on-chain before the refunder runs
    assert_eq!(
        ethflow_order
            .status(onchain.contracts(), ethflow_contract)
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
            .status(onchain.contracts(), ethflow_contract)
            .await,
        RefundStatus::Refunded
    );

    // Verify no refund TX was recorded (the refunder didn't process this order)
    let order = services.get_order(&order_id).await.unwrap();
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
/// This test needs actual settlement, which requires real-world timestamps
/// (autopilot validates against wall-clock time).
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

    // Anvil starts with a hardcoded timestamp of Jan 1, 2020 (see nodes/mod.rs).
    // The autopilot validates orders against real-world time, so valid_to must
    // exceed the current wall clock (which also exceeds anvil's simulated time).
    let valid_to = Utc::now().timestamp() as u32 + 3600;

    let (ethflow_order, order_id, valid_to) =
        EthflowOrderBuilder::new(&services, &onchain, &user, buy_token)
            .with_sell_amount(NonZeroU256::try_from(1u64.eth()).unwrap())
            .with_slippage_bps(300)
            .with_valid_to(valid_to)
            .create_and_index()
            .await;

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();

    wait_for_order_settlement(&services, &onchain, &order_id).await;

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

    let mut refund_service = RefundService::from_components(
        services.db().clone(),
        web3.clone(),
        vec![*ethflow_contract.address()],
        0, // min_validity_duration = 0 (permissive)
        0, // min_price_deviation_bps = 0 (permissive)
        refunder_account.signer,
        MAX_GAS_PRICE,
        START_PRIORITY_FEE_TIP,
        None,
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

/// Tests that the refunder can process orders from multiple ethflow contracts.
///
/// Orders won't settle because they use blockchain time for `valid_to`, making
/// them appear expired to the autopilot. This isolation allows the test to
/// focus on verifying multi-contract refund processing without settlement
/// complexity.
#[ignore]
#[tokio::test]
async fn local_node_refunder_multiple_ethflow_contracts() {
    run_test(refunder_multiple_ethflow_contracts).await;
}

async fn refunder_multiple_ethflow_contracts(web3: Web3) {
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

    let validity_duration = 600u32;
    let valid_to = timestamp_of_current_block_in_seconds(&web3.provider)
        .await
        .unwrap()
        + validity_duration;

    // Create first order on primary ethflow contract (index 0)
    let (ethflow_order, order_id, valid_to) =
        EthflowOrderBuilder::new(&services, &onchain, &user, buy_token)
            .with_slippage_bps(500)
            .with_valid_to(valid_to)
            .with_ethflow_index(0)
            .create_and_index()
            .await;

    // Create second order on secondary ethflow contract (index 1)
    let (ethflow_order_2, order_id_2, _) =
        EthflowOrderBuilder::new(&services, &onchain, &user, buy_token)
            .with_slippage_bps(500)
            .with_valid_to(valid_to)
            .with_ethflow_index(1)
            .create_and_index()
            .await;

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    let ethflow_contract_2 = onchain.contracts().ethflows.get(1).unwrap();

    advance_time_past_expiration(&web3, valid_to).await;

    let mut refund_service = RefundService::from_components(
        services.db().clone(),
        web3,
        vec![*ethflow_contract.address(), *ethflow_contract_2.address()],
        validity_duration as i64 / 2,
        10,
        refunder.signer,
        MAX_GAS_PRICE,
        START_PRIORITY_FEE_TIP,
        None,
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
