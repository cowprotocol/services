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
