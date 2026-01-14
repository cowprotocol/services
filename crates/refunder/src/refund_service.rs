//! Refund eligibility checks and batch submission.

use {
    crate::traits::{ChainRead, ChainWrite, DbRead, RefundStatus},
    alloy::primitives::{Address, B256},
    anyhow::Result,
    database::{OrderUid, ethflow_orders::EthOrderPlacement},
    futures::{StreamExt, stream},
    std::collections::HashMap,
};

const MAX_NUMBER_OF_UIDS_PER_REFUND_TX: usize = 30;

type CoWSwapEthFlowAddress = Address;

/// Filters `refundable_order_uids` by on-chain status, returning only orders
/// that need refund, grouped by EthFlow contract.
///
/// Excludes orders from unknown contracts, failed status queries, already
/// refunded orders, and owners that can't receive ETH.
async fn identify_uids_refunding_status<C: ChainRead>(
    chain: &C,
    refundable_order_uids: &[EthOrderPlacement],
) -> HashMap<CoWSwapEthFlowAddress, Vec<OrderUid>> {
    let known_ethflow_addresses = chain.ethflow_addresses();

    let futures = refundable_order_uids
        .iter()
        .filter_map(|eth_order_placement| {
            let ethflow_contract_address = Address::from_slice(&eth_order_placement.uid.0[32..52]);
            let is_known = known_ethflow_addresses.contains(&ethflow_contract_address);
            if !is_known {
                tracing::warn!(
                    uid = const_hex::encode_prefixed(eth_order_placement.uid.0),
                    ethflow = ?ethflow_contract_address,
                    "refunding orders from specific contract is not enabled",
                );
                return None;
            }
            Some((eth_order_placement, ethflow_contract_address))
        })
        .map(
            |(eth_order_placement, ethflow_contract_address)| async move {
                let order_hash: [u8; 32] = eth_order_placement.uid.0[0..32]
                    .try_into()
                    .expect("order_uid slice with incorrect length");
                let status = chain
                    .get_order_status(ethflow_contract_address, B256::from(order_hash))
                    .await;
                let status = match status {
                    Ok(status) => status,
                    Err(err) => {
                        tracing::error!(
                            uid =? B256::from(order_hash),
                            ?err,
                            "Error while getting the current onchain status of the order"
                        );
                        return None;
                    }
                };
                let status = match status {
                    RefundStatus::NotYetRefunded(owner) if !chain.can_receive_eth(owner).await => {
                        tracing::warn!(
                            uid = const_hex::encode_prefixed(eth_order_placement.uid.0),
                            ?owner,
                            "Order owner cannot receive ETH - marking as invalid"
                        );
                        RefundStatus::Invalid
                    }
                    other => other,
                };

                Some((eth_order_placement.uid, status, ethflow_contract_address))
            },
        );

    let uid_with_latest_refundablility = futures::future::join_all(futures).await;
    let mut to_be_refunded_uids = HashMap::<_, Vec<_>>::new();
    let mut invalid_uids = Vec::new();
    for (uid, status, contract_address) in uid_with_latest_refundablility.into_iter().flatten() {
        match status {
            RefundStatus::Refunded => (),
            RefundStatus::Invalid => invalid_uids.push(uid),
            RefundStatus::NotYetRefunded(_) => {
                to_be_refunded_uids
                    .entry(contract_address)
                    .or_default()
                    .push(uid);
            }
        }
    }
    if !invalid_uids.is_empty() {
        // In exceptional cases, e.g. if the refunder tries to refund orders from a
        // previous contract, the order_owners could be zero, or the owner cannot
        // receive ETH (e.g. EOF contracts or contracts with restrictive receive logic)
        tracing::warn!(
            "Skipping invalid orders (not created in current contract or owner cannot receive \
             ETH). Uids: {:?}",
            invalid_uids
        );
    }
    to_be_refunded_uids
}

pub struct RefundService<D, CR, CW>
where
    D: DbRead,
    CR: ChainRead,
    CW: ChainWrite,
{
    pub database: D,
    pub chain: CR,
    pub submitter: CW,
    pub min_validity_duration: i64,
    pub min_price_deviation: f64,
}

impl<D, CR, CW> RefundService<D, CR, CW>
where
    D: DbRead,
    CR: ChainRead,
    CW: ChainWrite,
{
    pub fn new(
        database: D,
        chain: CR,
        submitter: CW,
        min_validity_duration: i64,
        min_price_deviation: f64,
    ) -> Self {
        RefundService {
            database,
            chain,
            submitter,
            min_validity_duration,
            min_price_deviation,
        }
    }

    /// Fetches refundable orders from DB, validates on-chain, and submits batch
    /// refunds. Individual failures are logged and skipped.
    pub async fn try_to_refund_all_eligible_orders(&mut self) -> Result<()> {
        let refundable_order_uids = self.get_refundable_ethflow_orders_from_db().await?;

        let to_be_refunded_uids =
            identify_uids_refunding_status(&self.chain, &refundable_order_uids).await;

        self.send_out_refunding_tx(to_be_refunded_uids).await?;
        Ok(())
    }

    /// Fetches expired EthFlow orders that haven't been refunded, invalidated,
    /// or filled.
    pub async fn get_refundable_ethflow_orders_from_db(&self) -> Result<Vec<EthOrderPlacement>> {
        let block_time = self.chain.current_block_timestamp().await? as i64;

        self.database
            .get_refundable_orders(
                block_time,
                self.min_validity_duration,
                self.min_price_deviation,
            )
            .await
    }

    async fn send_out_refunding_tx(
        &mut self,
        uids_by_contract: HashMap<CoWSwapEthFlowAddress, Vec<OrderUid>>,
    ) -> Result<()> {
        if uids_by_contract.is_empty() {
            return Ok(());
        }

        // For each ethflow contract, issue a separate tx to refund
        for (contract, mut uids) in uids_by_contract.into_iter() {
            // only try to refund MAX_NUMBER_OF_UIDS_PER_REFUND_TX uids, in order to fit
            // into gas limit
            uids.truncate(MAX_NUMBER_OF_UIDS_PER_REFUND_TX);

            tracing::debug!("Trying to refund the following uids: {:?}", uids);

            let futures = uids.iter().map(|uid| {
                let (uid, database) = (*uid, &self.database);
                async move { database.get_ethflow_order_data(&uid).await }
            });
            let encoded_ethflow_orders: Vec<_> = stream::iter(futures)
                .buffer_unordered(10)
                .filter_map(|result| async {
                    result
                        .inspect_err(|err| tracing::error!(?err, "failed to get data from db"))
                        .ok()
                })
                .collect()
                .await;
            self.submitter
                .submit(&uids, encoded_ethflow_orders, contract)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::traits::{MockChainRead, MockChainWrite, MockDbRead},
        alloy::primitives::Address,
        anyhow::anyhow,
        contracts::alloy::CoWSwapEthFlow::EthFlowOrder,
        database::{byte_array::ByteArray, ethflow_orders::EthOrderPlacement},
        rstest::rstest,
        std::collections::{HashMap, HashSet},
    };

    /// Test addresses with semantic meaning for filtering logic.
    pub const KNOWN_ETHFLOW: Address = Address::repeat_byte(0x11);
    pub const KNOWN_ETHFLOW_2: Address = Address::repeat_byte(0x22);
    pub const EOA_OWNER: Address = Address::repeat_byte(0x44);
    pub const CONTRACT_REJECTING_ETH: Address = Address::repeat_byte(0x55);
    pub const UNKNOWN_ETHFLOW: Address = Address::repeat_byte(0x66);

    /// Asserts the expected number of orders per contract in a grouped result.
    ///
    /// # Panics
    /// Panics if the number of orders for the specified contract does not match
    /// the expected count, or if the set of UID suffixes differs.
    #[track_caller]
    fn assert_orders_by_contract(
        result: &HashMap<CoWSwapEthFlowAddress, Vec<OrderUid>>,
        contract: Address,
        expected_uid_suffixes: &[u8],
    ) {
        // Retrieve the order list for the contract (empty slice if missing)
        let orders = result.get(&contract).map(|v| v.as_slice()).unwrap_or(&[]);

        // Verify the count
        let actual_count = orders.len();
        let expected_count = expected_uid_suffixes.len();
        assert_eq!(
            actual_count, expected_count,
            "Expected {expected_count} orders for contract {contract}, got {actual_count}"
        );

        // Verify the UID suffixes (order‑independent)
        let actual_suffixes: HashSet<u8> = orders.iter().map(|uid| uid.0[31]).collect();
        let expected_suffixes: HashSet<u8> = expected_uid_suffixes.iter().copied().collect();

        assert_eq!(
            actual_suffixes, expected_suffixes,
            "Order uid_suffixes mismatch for contract {contract}"
        );
    }

    /// Builds an `EthOrderPlacement` with the given contract address embedded
    /// in the UID.
    ///
    /// # UID Structure (56 bytes total)
    ///
    /// The CoW Protocol Order UID has the following layout:
    /// - Bytes 0-31: Order hash (keccak256 of order data)
    /// - Bytes 32-51: Contract address (EthFlow contract that created the
    ///   order)
    /// - Bytes 52-55: Valid-to timestamp (big-endian u32)
    ///
    /// This function creates a test UID where:
    /// - `uid_suffix` is placed at byte 31 (end of order hash) for easy
    ///   identification
    /// - `contract_addr` occupies bytes 32-52 to simulate the EthFlow contract
    ///   address
    ///
    /// # Arguments
    /// - `uid_suffix`: A byte value placed at position 31 to distinguish test
    ///   orders
    /// - `contract_addr`: The EthFlow contract address to embed in bytes 32-52
    fn create_test_order_placement(uid_suffix: u8, contract_addr: Address) -> EthOrderPlacement {
        let mut uid_bytes = [0u8; 56];
        uid_bytes[31] = uid_suffix;
        uid_bytes[32..52].copy_from_slice(contract_addr.as_slice());
        EthOrderPlacement {
            uid: ByteArray(uid_bytes),
            valid_to: 1000,
        }
    }

    // -------------------------------------------------------------------------
    // Extension traits to reduce mock setup boilerplate
    // -------------------------------------------------------------------------

    trait MockChainReadExt {
        fn with_standard_refundable_setup(&mut self) -> &mut Self;
        fn with_block_timestamp(&mut self, timestamp: u32) -> &mut Self;
        fn with_ethflow_addresses(&mut self, addresses: Vec<Address>) -> &mut Self;
    }

    impl MockChainReadExt for MockChainRead {
        fn with_standard_refundable_setup(&mut self) -> &mut Self {
            self.expect_ethflow_addresses()
                .returning(|| vec![KNOWN_ETHFLOW]);
            self.expect_get_order_status()
                .returning(|_, _| Ok(RefundStatus::NotYetRefunded(EOA_OWNER)));
            self.expect_can_receive_eth().returning(|_| true);
            self
        }

        fn with_block_timestamp(&mut self, timestamp: u32) -> &mut Self {
            self.expect_current_block_timestamp()
                .returning(move || Ok(timestamp));
            self
        }

        fn with_ethflow_addresses(&mut self, addresses: Vec<Address>) -> &mut Self {
            self.expect_ethflow_addresses()
                .returning(move || addresses.clone());
            self
        }
    }

    trait MockDbReadExt {
        fn with_ethflow_order_data_success(&mut self) -> &mut Self;
    }

    impl MockDbReadExt for MockDbRead {
        fn with_ethflow_order_data_success(&mut self) -> &mut Self {
            self.expect_get_ethflow_order_data()
                .returning(|_| Ok(EthFlowOrder::Data::default()));
            self
        }
    }

    // -------------------------------------------------------------------------
    // Tests
    // -------------------------------------------------------------------------

    /// Orders with owners that cannot receive ETH are filtered out.
    #[rstest]
    #[case::eoa_can_receive_eth(EOA_OWNER, true)]
    #[case::contract_rejects_eth(CONTRACT_REJECTING_ETH, false)]
    #[tokio::test]
    async fn test_eth_receivability_filtering(#[case] owner: Address, #[case] can_receive: bool) {
        let mut mock_chain = MockChainRead::new();

        // Configure the known EthFlow contracts so orders from KNOWN_ETHFLOW pass the
        // allowlist check
        mock_chain
            .expect_ethflow_addresses()
            .returning(|| vec![KNOWN_ETHFLOW]);

        // All orders report as "not yet refunded" with the parameterized owner address
        // This sets up the precondition for testing the ETH receivability check
        mock_chain
            .expect_get_order_status()
            .returning(move |_, _| Ok(RefundStatus::NotYetRefunded(owner)));

        // Return parameterized ETH receivability result to test both EOA (can receive)
        // and contract-rejecting-ETH (cannot receive) scenarios
        mock_chain
            .expect_can_receive_eth()
            .withf(move |addr| *addr == owner)
            .returning(move |_| can_receive);

        let order = create_test_order_placement(1, KNOWN_ETHFLOW);
        let result = identify_uids_refunding_status(&mock_chain, &[order]).await;

        let expected_orders: &[u8] = if can_receive { &[1] } else { &[] };
        assert_orders_by_contract(&result, KNOWN_ETHFLOW, expected_orders);
    }

    /// Orders from unknown EthFlow contracts are filtered out.
    #[rstest]
    #[case::known_contract_included(KNOWN_ETHFLOW)]
    #[case::unknown_contract_filtered(UNKNOWN_ETHFLOW)]
    #[tokio::test]
    async fn test_ethflow_contract_filtering(#[case] contract: Address) {
        let mut mock_chain = MockChainRead::new();
        mock_chain.with_standard_refundable_setup();

        let order = create_test_order_placement(1, contract);
        let result = identify_uids_refunding_status(&mock_chain, &[order]).await;

        let expected_suffixes: &[u8] = if contract == KNOWN_ETHFLOW { &[1] } else { &[] };
        assert_orders_by_contract(&result, contract, expected_suffixes);
    }

    /// Orders with non-refundable status or status query errors are excluded.
    #[rstest]
    #[case::already_refunded(Some(RefundStatus::Refunded))]
    #[case::invalid_order(Some(RefundStatus::Invalid))]
    #[case::status_query_error(None)]
    #[tokio::test]
    async fn test_non_refundable_status_excludes_order(#[case] status: Option<RefundStatus>) {
        let mut mock_chain = MockChainRead::new();

        // Allow the order through the allowlist check
        mock_chain
            .expect_ethflow_addresses()
            .returning(|| vec![KNOWN_ETHFLOW]);

        // Return the parameterized status (Refunded, Invalid, or Error) to verify
        // that orders with non-refundable statuses are excluded from the result
        mock_chain
            .expect_get_order_status()
            .returning(move |_, _| status.ok_or(anyhow!("RPC error")));

        let order = create_test_order_placement(1, KNOWN_ETHFLOW);
        let result = identify_uids_refunding_status(&mock_chain, &[order]).await;

        assert!(result.is_empty());
    }

    /// A single order is forwarded to the submitter with correct arguments.
    #[tokio::test]
    async fn test_send_out_refunding_tx_calls_submitter() {
        let order = create_test_order_placement(1, KNOWN_ETHFLOW);
        let uid = order.uid;

        let mut mock_db = MockDbRead::new();
        mock_db.with_ethflow_order_data_success();

        let mock_chain = MockChainRead::new();

        let mut mock_submitter = MockChainWrite::new();
        mock_submitter
            .expect_submit()
            .times(1)
            .withf(|uids, orders, contract| {
                uids.len() == 1 && orders.len() == 1 && *contract == KNOWN_ETHFLOW
            })
            .returning(|_, _, _| Ok(()));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let mut uids_by_contract = HashMap::new();
        uids_by_contract.insert(KNOWN_ETHFLOW, vec![uid]);

        let result = service.send_out_refunding_tx(uids_by_contract).await;
        assert!(result.is_ok());
    }

    /// Empty order map does not trigger any submission.
    #[tokio::test]
    async fn test_send_out_refunding_tx_empty_map_skips_submission() {
        // No expectations needed for DB or chain because empty input short-circuits
        // before any DB and chain calls
        let mock_db = MockDbRead::new();
        let mock_chain = MockChainRead::new();

        // Submitter has no expectations: test will fail if submit is called,
        // verifying that empty input correctly skips submission
        let mock_submitter = MockChainWrite::new();

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let result = service.send_out_refunding_tx(HashMap::new()).await;
        assert!(result.is_ok());
    }

    /// Orders are capped at `MAX_NUMBER_OF_UIDS_PER_REFUND_TX` per contract.
    #[rstest]
    #[case::at_max_no_truncation(
        MAX_NUMBER_OF_UIDS_PER_REFUND_TX,
        MAX_NUMBER_OF_UIDS_PER_REFUND_TX
    )]
    #[case::above_max_truncates(35, MAX_NUMBER_OF_UIDS_PER_REFUND_TX)]
    #[tokio::test]
    async fn test_send_out_refunding_tx_order_count_boundary(
        #[case] input_count: usize,
        #[case] expected_count: usize,
    ) {
        let mut mock_db = MockDbRead::new();
        mock_db.with_ethflow_order_data_success();

        let mock_chain = MockChainRead::new();

        let mut mock_submitter = MockChainWrite::new();
        mock_submitter
            .expect_submit()
            .times(1)
            .withf(move |uids, orders, _| {
                uids.len() == expected_count && orders.len() == expected_count
            })
            .returning(|_, _, _| Ok(()));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let mut uids_by_contract = HashMap::new();
        let uids = (0..input_count as u8)
            .map(|i| create_test_order_placement(i, KNOWN_ETHFLOW).uid)
            .collect();
        uids_by_contract.insert(KNOWN_ETHFLOW, uids);

        let result = service.send_out_refunding_tx(uids_by_contract).await;
        assert!(result.is_ok());
    }

    /// Orders from multiple contracts trigger separate submissions.
    #[tokio::test]
    async fn test_send_out_refunding_tx_multiple_contracts() {
        let mut mock_db = MockDbRead::new();
        mock_db.with_ethflow_order_data_success();

        let mock_chain = MockChainRead::new();

        let mut mock_submitter = MockChainWrite::new();

        // Expect exactly 2 submissions because orders are grouped by contract,
        // and each contract gets its own refund transaction
        mock_submitter
            .expect_submit()
            .times(2)
            .returning(|_, _, _| Ok(()));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let uid1 = create_test_order_placement(1, KNOWN_ETHFLOW).uid;
        let uid2 = create_test_order_placement(2, KNOWN_ETHFLOW_2).uid;
        let mut uids_by_contract = HashMap::new();
        uids_by_contract.insert(KNOWN_ETHFLOW, vec![uid1]);
        uids_by_contract.insert(KNOWN_ETHFLOW_2, vec![uid2]);

        let result = service.send_out_refunding_tx(uids_by_contract).await;
        assert!(result.is_ok());
    }

    /// DB errors for individual orders are skipped; other orders proceed.
    ///
    /// # Current Behavior (documented, not necessarily ideal)
    ///
    /// When a DB lookup fails for an order:
    /// - The error is logged and the order data is excluded from the submission
    /// - However, the UID is still included in the submission
    ///
    /// This means `submit` receives:
    /// - `uids`: ALL original UIDs (including those with failed lookups)
    /// - `orders`: Only the order data for successful lookups
    ///
    /// This creates a mismatch between UIDs and order data. See the TODO in
    /// `test_send_out_refunding_tx_all_db_calls_fail_still_submits` for
    /// discussion of potential fixes.
    #[tokio::test]
    async fn test_send_out_refunding_tx_db_error_skips_order() {
        let uid1 = create_test_order_placement(1, KNOWN_ETHFLOW).uid;
        let uid2 = create_test_order_placement(2, KNOWN_ETHFLOW).uid;

        let mut mock_db = MockDbRead::new();

        // First order (uid_suffix=1) fails DB lookup to test error handling
        mock_db
            .expect_get_ethflow_order_data()
            .withf(|uid| uid.0[31] == 1)
            .returning(|_| Err(anyhow!("DB error")));

        // Second order (uid_suffix=2) succeeds to verify partial success behavior
        mock_db
            .expect_get_ethflow_order_data()
            .withf(|uid| uid.0[31] == 2)
            .returning(|_| Ok(EthFlowOrder::Data::default()));

        let mock_chain = MockChainRead::new();

        let mut mock_submitter = MockChainWrite::new();

        // Current behavior: ALL UIDs are passed, but only successful order data.
        // - uids contains both uid1 (suffix=1) and uid2 (suffix=2)
        // - orders contains only 1 entry (from uid2's successful lookup)
        mock_submitter
            .expect_submit()
            .times(1)
            .withf(|uids, orders, _| {
                let has_both_uids = uids.len() == 2
                    && uids.iter().any(|uid| uid.0[31] == 1)
                    && uids.iter().any(|uid| uid.0[31] == 2);
                let has_one_order = orders.len() == 1;
                has_both_uids && has_one_order
            })
            .returning(|_, _, _| Ok(()));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let mut uids_by_contract = HashMap::new();
        uids_by_contract.insert(KNOWN_ETHFLOW, vec![uid1, uid2]);

        let result = service.send_out_refunding_tx(uids_by_contract).await;
        assert!(result.is_ok());
    }

    /// If every DB lookup fails, we still call the submitter with the original
    /// UIDs but without any order data.
    ///
    /// What actually happens:
    /// - Each failed order‑data fetch is logged and ignored (it doesn't stop
    ///   the whole batch).
    /// - The submitter gets the same list of UIDs we started with, but the
    ///   `orders` slice may be empty (or contain fewer entries) because some or
    ///   all lookups failed.
    ///
    /// TODO: Is this the behavior we really want? Submitting a refund that
    /// contains UIDs but no order details feels off. Possible fixes:
    /// 1. Skip the submission entirely when `encoded_ethflow_orders` is empty.
    /// 2. Return an error if *all* order‑data lookups fail.
    /// 3. Filter the UID list so it only includes IDs with successful lookups.
    ///
    /// NOTE: This test complements
    /// `test_send_out_refunding_tx_db_error_skips_order`. That test covers
    /// partial DB failure (some lookups succeed); this one covers
    /// total DB failure (all lookups fail). Together they verify that DB errors
    /// are non-fatal and UIDs are always preserved regardless of lookup
    /// success.
    #[tokio::test]
    async fn test_send_out_refunding_tx_all_db_calls_fail_still_submits() {
        let uid1 = create_test_order_placement(1, KNOWN_ETHFLOW).uid;
        let uid2 = create_test_order_placement(2, KNOWN_ETHFLOW).uid;

        let mut mock_db = MockDbRead::new();

        // All DB lookups fail to test edge case where no order data is available
        mock_db
            .expect_get_ethflow_order_data()
            .returning(|_| Err(anyhow!("DB connection lost")));

        let mock_chain = MockChainRead::new();

        let mut mock_submitter = MockChainWrite::new();

        // Verify submission still happens with original UIDs but empty orders list
        // This documents current (possibly unintended) behavior where UIDs and orders
        // mismatch
        mock_submitter
            .expect_submit()
            .times(1)
            .withf(|uids, orders, contract| {
                // UIDs are preserved, but orders is empty because all DB lookups failed
                uids.len() == 2 && orders.is_empty() && *contract == KNOWN_ETHFLOW
            })
            .returning(|_, _, _| Ok(()));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let mut uids_by_contract = HashMap::new();
        uids_by_contract.insert(KNOWN_ETHFLOW, vec![uid1, uid2]);

        let result = service.send_out_refunding_tx(uids_by_contract).await;
        assert!(result.is_ok());
    }

    /// Submitter error on first contract short-circuits; remaining contracts
    /// are not attempted.
    ///
    /// NOTE: HashMap iteration order is non-deterministic, so we cannot predict
    /// which contract will be processed first. This test verifies that:
    /// 1. The error propagates (result is Err)
    /// 2. Only one submission is attempted (times(1))
    ///
    /// The test remains valid regardless of iteration order because both
    /// contracts would fail with the same error.
    #[tokio::test]
    async fn test_send_out_refunding_tx_error_short_circuits() {
        let mut mock_db = MockDbRead::new();

        // Return order data successfully; the error will come from submission
        mock_db
            .expect_get_ethflow_order_data()
            .returning(|_| Ok(EthFlowOrder::Data::default()));

        let mock_chain = MockChainRead::new();

        let mut mock_submitter = MockChainWrite::new();

        // Fail on first submission to verify error propagation stops processing
        // Due to HashMap's non-deterministic iteration order, we cannot predict
        // which contract will be attempted first, but we know only one will be tried
        mock_submitter
            .expect_submit()
            .times(1)
            .returning(|_, _, _| Err(anyhow!("Submission failed")));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let uid1 = create_test_order_placement(1, KNOWN_ETHFLOW).uid;
        let uid2 = create_test_order_placement(2, KNOWN_ETHFLOW_2).uid;
        let mut uids_by_contract = HashMap::new();
        uids_by_contract.insert(KNOWN_ETHFLOW, vec![uid1]);
        uids_by_contract.insert(KNOWN_ETHFLOW_2, vec![uid2]);

        let result = service.send_out_refunding_tx(uids_by_contract).await;
        assert!(result.is_err());
    }

    /// An eligible order is fetched, validated, and submitted for refund.
    #[tokio::test]
    async fn test_try_to_refund_happy_path() {
        let order = create_test_order_placement(1, KNOWN_ETHFLOW);

        let mut mock_db = MockDbRead::new();
        mock_db
            .expect_get_refundable_orders()
            .returning(move |_, _, _| Ok(vec![order.clone()]));
        mock_db.with_ethflow_order_data_success();

        let mut mock_chain = MockChainRead::new();
        mock_chain
            .with_block_timestamp(1000)
            .with_standard_refundable_setup();

        let mut mock_submitter = MockChainWrite::new();
        mock_submitter.expect_submit().times(1).returning(|_, _, _| Ok(()));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let result = service.try_to_refund_all_eligible_orders().await;
        assert!(result.is_ok());
    }

    /// Empty database result does not trigger any submission.
    #[tokio::test]
    async fn test_try_to_refund_empty_db_result() {
        let mut mock_db = MockDbRead::new();
        mock_db
            .expect_get_refundable_orders()
            .returning(|_, _, _| Ok(vec![]));

        let mut mock_chain = MockChainRead::new();
        mock_chain
            .with_block_timestamp(1000)
            .with_ethflow_addresses(vec![KNOWN_ETHFLOW]);

        // Submitter has no expectations: test fails if submit is called
        let mock_submitter = MockChainWrite::new();

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let result = service.try_to_refund_all_eligible_orders().await;
        assert!(result.is_ok());
    }

    /// When some orders are already refunded on-chain, only pending orders are
    /// submitted.
    #[tokio::test]
    async fn test_try_to_refund_mixed_orders() {
        let order_valid = create_test_order_placement(1, KNOWN_ETHFLOW);
        let order_refunded = create_test_order_placement(2, KNOWN_ETHFLOW);

        let mut mock_db = MockDbRead::new();

        // Return two orders from DB: one still needs refund, one already refunded
        // on-chain
        mock_db
            .expect_get_refundable_orders()
            .returning(move |_, _, _| Ok(vec![order_valid.clone(), order_refunded.clone()]));

        // Return order data for the order that passes on-chain validation
        mock_db
            .expect_get_ethflow_order_data()
            .returning(|_| Ok(EthFlowOrder::Data::default()));

        let mut mock_chain = MockChainRead::new();

        // Block timestamp for DB query
        mock_chain
            .expect_current_block_timestamp()
            .returning(|| Ok(1000));

        // Configure known EthFlow contracts
        mock_chain
            .expect_ethflow_addresses()
            .returning(|| vec![KNOWN_ETHFLOW]);

        // Order 1 (uid_suffix=1) is eligible for refund
        mock_chain
            .expect_get_order_status()
            .withf(|_, order_hash| order_hash.0[31] == 1)
            .returning(|_, _| Ok(RefundStatus::NotYetRefunded(EOA_OWNER)));

        // Order 2 (uid_suffix=2) was already refunded on-chain, should be filtered out
        mock_chain
            .expect_get_order_status()
            .withf(|_, order_hash| order_hash.0[31] == 2)
            .returning(|_, _| Ok(RefundStatus::Refunded));

        // Owner can receive ETH
        mock_chain.expect_can_receive_eth().returning(|_| true);

        let mut mock_submitter = MockChainWrite::new();

        // Only 1 order should be submitted (order 2 is filtered out as already
        // refunded)
        mock_submitter
            .expect_submit()
            .times(1)
            .withf(|uids, _, _| uids.len() == 1)
            .returning(|_, _, _| Ok(()));

        let mut service = RefundService::new(mock_db, mock_chain, mock_submitter, 3600, 0.01);

        let result = service.try_to_refund_all_eligible_orders().await;
        assert!(result.is_ok());
    }

    /// Orders are grouped by their originating EthFlow contract.
    #[tokio::test]
    async fn test_identify_groups_orders_by_contract() {
        let order1 = create_test_order_placement(1, KNOWN_ETHFLOW);
        let order2 = create_test_order_placement(2, KNOWN_ETHFLOW);
        let order3 = create_test_order_placement(3, KNOWN_ETHFLOW_2);

        let mut mock_chain = MockChainRead::new();
        mock_chain.with_ethflow_addresses(vec![KNOWN_ETHFLOW, KNOWN_ETHFLOW_2]);
        mock_chain
            .expect_get_order_status()
            .returning(|_, _| Ok(RefundStatus::NotYetRefunded(EOA_OWNER)));
        mock_chain.expect_can_receive_eth().returning(|_| true);

        let result = identify_uids_refunding_status(&mock_chain, &[order1, order2, order3]).await;

        assert_eq!(result.len(), 2);
        assert_orders_by_contract(&result, KNOWN_ETHFLOW, &[1, 2]);
        assert_orders_by_contract(&result, KNOWN_ETHFLOW_2, &[3]);
    }

    /// Empty input returns empty result.
    #[tokio::test]
    async fn test_identify_empty_input() {
        let mut mock_chain = MockChainRead::new();
        mock_chain.with_ethflow_addresses(vec![KNOWN_ETHFLOW]);

        let result = identify_uids_refunding_status(&mock_chain, &[]).await;

        assert!(result.is_empty());
    }

    /// When multiple status queries fail, all failed orders are excluded.
    #[tokio::test]
    async fn test_multiple_status_query_failures() {
        let order1 = create_test_order_placement(1, KNOWN_ETHFLOW);
        let order2 = create_test_order_placement(2, KNOWN_ETHFLOW);
        let order3 = create_test_order_placement(3, KNOWN_ETHFLOW);

        let mut mock_chain = MockChainRead::new();

        // All orders pass the allowlist check
        mock_chain
            .expect_ethflow_addresses()
            .returning(|| vec![KNOWN_ETHFLOW]);

        // Orders 1 and 2 fail with RPC errors to test partial failure handling
        // Order 3 succeeds to verify successful orders are still processed
        mock_chain
            .expect_get_order_status()
            .returning(|_, order_hash| match order_hash.0[31] {
                1 | 2 => Err(anyhow!("RPC timeout")),
                3 => Ok(RefundStatus::NotYetRefunded(EOA_OWNER)),
                _ => panic!("unexpected order_hash"),
            });

        // Owner can receive ETH (only relevant for order 3 which passes status check)
        mock_chain.expect_can_receive_eth().returning(|_| true);

        let result = identify_uids_refunding_status(&mock_chain, &[order1, order2, order3]).await;

        // Only order 3 should be included (orders 1 and 2 failed status check)
        assert_orders_by_contract(&result, KNOWN_ETHFLOW, &[3]);
    }
}
