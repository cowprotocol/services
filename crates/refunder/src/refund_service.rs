use {
    crate::submitter::Submitter,
    alloy::{
        network::TxSigner,
        primitives::{Address, B256, Signature},
        providers::Provider,
        rpc::types::TransactionRequest,
    },
    anyhow::{Context, Result, anyhow},
    contracts::alloy::CoWSwapEthFlow,
    database::{
        OrderUid,
        ethflow_orders::{EthOrderPlacement, read_order, refundable_orders},
        orders::read_order as read_db_order,
    },
    ethrpc::{Web3, block_stream::timestamp_of_current_block_in_seconds},
    futures::{StreamExt, stream},
    number::conversions::big_decimal_to_u256,
    shared::gas_price_estimation::eth_node::NodeGasPriceEstimator,
    sqlx::PgPool,
    std::{collections::HashMap, time::Duration},
};

pub const NO_OWNER: Address = Address::ZERO;
pub const INVALIDATED_OWNER: Address = Address::repeat_byte(0xff);
const MAX_NUMBER_OF_UIDS_PER_REFUND_TX: usize = 30;

type CoWSwapEthFlowAddress = Address;

pub struct RefundService {
    pub db: PgPool,
    pub web3: Web3,
    pub ethflow_contracts: Vec<CoWSwapEthFlow::Instance>,
    pub min_validity_duration: i64,
    pub min_price_deviation: f64,
    pub submitter: Submitter,
    pub max_gas_price: u64,
    pub start_priority_fee_tip: u64,
    pub lookback_time: Option<Duration>,
}

/// Status of an EthFlow order refund eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundStatus {
    /// Order has already been refunded or cancelled.
    Refunded,
    /// Order is still active and eligible for refund, with the given owner
    /// address.
    NotYetRefunded(Address),
    /// Order is invalid (never created, already freed, or owner cannot receive
    /// ETH).
    Invalid,
}

impl From<CoWSwapEthFlow::CoWSwapEthFlow::ordersReturn> for RefundStatus {
    fn from(value: CoWSwapEthFlow::CoWSwapEthFlow::ordersReturn) -> Self {
        match value.owner {
            NO_OWNER => Self::Invalid,
            INVALIDATED_OWNER => Self::Refunded,
            owner => Self::NotYetRefunded(owner),
        }
    }
}

impl RefundService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: PgPool,
        web3: Web3,
        ethflow_contracts: Vec<CoWSwapEthFlow::Instance>,
        min_validity_duration: i64,
        min_price_deviation_bps: i64,
        signer: Box<dyn TxSigner<Signature> + Send + Sync + 'static>,
        max_gas_price: u64,
        start_priority_fee_tip: u64,
        lookback_time: Option<Duration>,
    ) -> Self {
        let signer_address = signer.address();
        let gas_estimator = Box::new(NodeGasPriceEstimator::new(web3.alloy.clone()));
        web3.wallet.register_signer(signer);
        RefundService {
            db,
            web3: web3.clone(),
            ethflow_contracts,
            min_validity_duration,
            min_price_deviation: min_price_deviation_bps as f64 / 10000f64,
            max_gas_price,
            start_priority_fee_tip,
            submitter: Submitter {
                web3,
                signer_address,
                gas_estimator,
                gas_parameters_of_last_tx: None,
                nonce_of_last_submission: None,
                max_gas_price,
                start_priority_fee_tip,
            },
            lookback_time,
        }
    }

    pub async fn try_to_refund_all_eligible_orders(&mut self) -> Result<()> {
        let refundable_order_uids = self.get_refundable_ethflow_orders_from_db().await?;

        let to_be_refunded_uids = self
            .identify_uids_refunding_status_via_web3_calls(refundable_order_uids)
            .await;

        self.send_out_refunding_tx(to_be_refunded_uids).await?;
        Ok(())
    }

    pub async fn get_refundable_ethflow_orders_from_db(&self) -> Result<Vec<EthOrderPlacement>> {
        let block_time = timestamp_of_current_block_in_seconds(&self.web3.alloy).await? as i64;

        let mut ex = self.db.acquire().await?;
        refundable_orders(
            &mut ex,
            block_time,
            self.min_validity_duration,
            self.min_price_deviation,
            self.lookback_time,
        )
        .await
        .map_err(|err| {
            anyhow!(
                "Error while retrieving the refundable ethflow orders from db: {:?}",
                err
            )
        })
    }

    /// Checks if an address can receive ETH by simulating a small transfer.
    /// Returns true for EOAs and contracts with working receive/fallback
    /// functions.
    async fn can_receive_eth(&self, address: Address) -> bool {
        // Try to estimate gas for sending a minimal amount of ETH
        let tx = TransactionRequest::default()
            .to(address)
            .value(alloy::primitives::U256::from(1));

        self.web3
            .alloy
            .estimate_gas(tx)
            .await
            .inspect_err(|err| {
                tracing::warn!(
                    ?address,
                    ?err,
                    "Address cannot receive ETH - will skip refund"
                );
            })
            .is_ok()
    }

    async fn identify_uids_refunding_status_via_web3_calls(
        &self,
        refundable_order_uids: Vec<EthOrderPlacement>,
    ) -> HashMap<CoWSwapEthFlowAddress, Vec<OrderUid>> {
        let futures = refundable_order_uids
            .into_iter()
            .filter_map(|eth_order_placement| {
                // Owner of the ethflow order is always the ethflow contract itself
                let ethflow_contract_address =
                    Address::from_slice(&eth_order_placement.uid.0[32..52]);
                let ethflow_contract = self
                    .ethflow_contracts
                    .iter()
                    .find(|contract| *contract.address() == ethflow_contract_address);
                if ethflow_contract.is_none() {
                    tracing::warn!(
                        uid = const_hex::encode_prefixed(eth_order_placement.uid.0),
                        ethflow = ?ethflow_contract_address,
                        "refunding orders from specific contract is not enabled",
                    );
                }
                ethflow_contract.map(|contract| (eth_order_placement, contract))
            })
            .map(|(eth_order_placement, ethflow_contract)| async move {
                let order_hash: [u8; 32] = eth_order_placement.uid.0[0..32]
                    .try_into()
                    .expect("order_uid slice with incorrect length");
                let order = ethflow_contract.orders(order_hash.into()).call().await;
                let order_owner = match order {
                    Ok(order) => order.owner,
                    Err(err) => {
                        tracing::error!(
                            uid =? B256::from(order_hash),
                            ?err,
                            "Error while getting the current onchain status ot the order"
                        );
                        return None;
                    }
                };
                let refund_status = if order_owner == INVALIDATED_OWNER {
                    RefundStatus::Refunded
                } else if order_owner == NO_OWNER {
                    RefundStatus::Invalid
                } else if !self.can_receive_eth(order_owner).await {
                    tracing::warn!(
                        uid = const_hex::encode_prefixed(eth_order_placement.uid.0),
                        owner = ?order_owner,
                        "Order owner cannot receive ETH - marking as invalid"
                    );
                    RefundStatus::Invalid
                } else {
                    RefundStatus::NotYetRefunded(order_owner)
                };

                Some((eth_order_placement.uid, refund_status, ethflow_contract))
            });

        let uid_with_latest_refundablility = futures::future::join_all(futures).await;
        let mut to_be_refunded_uids = HashMap::<_, Vec<_>>::new();
        let mut invalid_uids = Vec::new();
        for (uid, refund_status, ethflow_contract) in
            uid_with_latest_refundablility.into_iter().flatten()
        {
            match refund_status {
                RefundStatus::Refunded => (),
                RefundStatus::Invalid => invalid_uids.push(uid),
                RefundStatus::NotYetRefunded(_) => {
                    to_be_refunded_uids
                        .entry(*ethflow_contract.address())
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

    async fn get_ethflow_data_from_db(
        &self,
        uid: &OrderUid,
    ) -> Result<CoWSwapEthFlow::EthFlowOrder::Data> {
        let mut ex = self.db.acquire().await.context("acquire")?;
        let order = read_db_order(&mut ex, uid)
            .await
            .context("read order")?
            .context("missing order")?;
        let ethflow_order = read_order(&mut ex, uid)
            .await
            .context("read ethflow order")?
            .context("missing ethflow order")?;

        Ok(CoWSwapEthFlow::EthFlowOrder::Data {
            buyToken: Address::from(order.buy_token.0),
            // ethflow orders have always a receiver. It's enforced by the contract.
            receiver: Address::from(order.receiver.unwrap().0),
            sellAmount: big_decimal_to_u256(&order.sell_amount).unwrap(),
            buyAmount: big_decimal_to_u256(&order.buy_amount).unwrap(),
            appData: order.app_data.0.into(),
            feeAmount: big_decimal_to_u256(&order.fee_amount).unwrap(),
            validTo: ethflow_order.valid_to as u32,
            partiallyFillable: order.partially_fillable,
            quoteId: 0i64, // quoteId is not important for refunding and will be ignored
        })
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
                let (uid, self_) = (*uid, &self);
                async move {
                    self_
                        .get_ethflow_data_from_db(&uid)
                        .await
                        .context(format!("uid {uid:?}"))
                }
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
                .submit(uids, encoded_ethflow_orders, contract)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::address,
        shared::gas_price_estimation::eth_node::NodeGasPriceEstimator,
    };

    /// Creates a minimal RefundService for testing purposes.
    fn new_test_service(web3: Web3) -> RefundService {
        RefundService {
            db: PgPool::connect_lazy("postgresql://").unwrap(),
            web3: web3.clone(),
            ethflow_contracts: vec![],
            min_validity_duration: 0,
            min_price_deviation: 0.0,
            max_gas_price: 0,
            start_priority_fee_tip: 0,
            submitter: Submitter {
                web3: web3.clone(),
                signer_address: Address::ZERO,
                gas_estimator: Box::new(NodeGasPriceEstimator::new(web3.alloy.clone())),
                gas_parameters_of_last_tx: None,
                nonce_of_last_submission: None,
                max_gas_price: 0,
                start_priority_fee_tip: 0,
            },
            lookback_time: None,
        }
    }

    /// Verifies that `can_receive_eth()` correctly identifies addresses that
    /// cannot receive ETH transfers. Some smart contracts reject ETH transfers
    /// (e.g., EOF contracts or contracts without receive/fallback functions),
    /// which causes batch refunds to fail with EthTransferFailed errors.
    ///
    /// This test uses a real Sepolia EOF contract address that rejects ETH and
    /// compares it against a normal EOA to ensure the filtering logic works.
    #[tokio::test]
    #[ignore] // Run with: cargo test --package refunder --lib test_problematic_sepolia_address -- --ignored
    async fn test_problematic_sepolia_address() {
        let web3 = Web3::new_from_url("https://ethereum-sepolia-rpc.publicnode.com");
        let service = new_test_service(web3);

        // EOF contract that cannot receive ETH (0xef01... bytecode prefix)
        let problematic = address!("0x66C9152339ce05EE0C8A8eff9EeF8230AbFe8350");

        // Normal EOA for comparison
        let working = address!("0x5b485e4431853F82d89dba68220A422CC17cE024");

        // Test that can_receive_eth correctly identifies the problematic address
        assert!(
            !service.can_receive_eth(problematic).await,
            "EOF contract should be identified as unable to receive ETH"
        );

        // Test that can_receive_eth correctly identifies a working address
        assert!(
            service.can_receive_eth(working).await,
            "Normal EOA should be identified as able to receive ETH"
        );
    }
}
