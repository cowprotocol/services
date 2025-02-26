use {
    super::ethflow_order::{EncodedEthflowOrder, EthflowOrder, order_to_ethflow_data},
    crate::submitter::Submitter,
    anyhow::{Context, Result, anyhow},
    contracts::CoWSwapEthFlow,
    database::{
        OrderUid,
        ethflow_orders::{EthOrderPlacement, read_order, refundable_orders},
        orders::read_order as read_db_order,
    },
    ethcontract::{Account, H160, H256},
    ethrpc::{Web3, block_stream::timestamp_of_current_block_in_seconds},
    futures::{StreamExt, stream},
    sqlx::PgPool,
    std::collections::HashMap,
};

pub const NO_OWNER: H160 = H160([0u8; 20]);
pub const INVALIDATED_OWNER: H160 = H160([255u8; 20]);
const MAX_NUMBER_OF_UIDS_PER_REFUND_TX: usize = 30;

type CoWSwapEthFlowAddress = H160;

pub struct RefundService {
    pub db: PgPool,
    pub web3: Web3,
    pub ethflow_contracts: Vec<CoWSwapEthFlow>,
    pub min_validity_duration: i64,
    pub min_slippage: f64,
    pub submitter: Submitter,
}

#[derive(Debug, Eq, PartialEq)]
enum RefundStatus {
    Refunded,
    NotYetRefunded,
    Invalid,
}

impl RefundService {
    pub fn new(
        db: PgPool,
        web3: Web3,
        ethflow_contracts: Vec<CoWSwapEthFlow>,
        min_validity_duration: i64,
        min_slippage_bps: u64,
        account: Account,
    ) -> Self {
        RefundService {
            db,
            web3: web3.clone(),
            ethflow_contracts,
            min_validity_duration,
            min_slippage: min_slippage_bps as f64 / 10000f64,
            submitter: Submitter {
                web3: web3.clone(),
                account,
                gas_estimator: Box::new(web3),
                gas_parameters_of_last_tx: None,
                nonce_of_last_submission: None,
            },
        }
    }

    pub async fn try_to_refund_all_eligble_orders(&mut self) -> Result<()> {
        let refundable_order_uids = self.get_refundable_ethflow_orders_from_db().await?;

        let to_be_refunded_uids = self
            .identify_uids_refunding_status_via_web3_calls(refundable_order_uids)
            .await;

        self.send_out_refunding_tx(to_be_refunded_uids).await?;
        Ok(())
    }

    pub async fn get_refundable_ethflow_orders_from_db(&self) -> Result<Vec<EthOrderPlacement>> {
        let block_time = timestamp_of_current_block_in_seconds(&self.web3).await? as i64;

        let mut ex = self.db.acquire().await?;
        refundable_orders(
            &mut ex,
            block_time,
            self.min_validity_duration,
            self.min_slippage,
        )
        .await
        .map_err(|err| {
            anyhow!(
                "Error while retrieving the refundable ethflow orders from db: {:?}",
                err
            )
        })
    }

    async fn identify_uids_refunding_status_via_web3_calls(
        &self,
        refundable_order_uids: Vec<EthOrderPlacement>,
    ) -> HashMap<CoWSwapEthFlowAddress, Vec<OrderUid>> {
        let futures = refundable_order_uids
            .into_iter()
            .filter_map(|eth_order_placement| {
                // Owner of the ethflow order is always the ethflow contract itself
                let ethflow_contract_address = H160(
                    eth_order_placement.uid.0[32..52]
                        .try_into()
                        .expect("order_uid slice with incorrect length"),
                );
                let ethflow_contract = self
                    .ethflow_contracts
                    .iter()
                    .find(|contract| contract.address() == ethflow_contract_address);
                if ethflow_contract.is_none() {
                    tracing::warn!(
                        uid = format!("0x{}", hex::encode(eth_order_placement.uid.0)),
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
                let order = ethflow_contract
                    .orders(ethcontract::tokens::Bytes(order_hash))
                    .call()
                    .await;
                let order_owner = match order {
                    Ok(order) => Some(order.0),
                    Err(err) => {
                        tracing::error!(
                            uid =? H256(order_hash),
                            ?err,
                            "Error while getting the current onchain status ot the order"
                        );
                        return None;
                    }
                };
                let refund_status = match order_owner {
                    Some(bytes) if bytes == INVALIDATED_OWNER => RefundStatus::Refunded,
                    Some(bytes) if bytes == NO_OWNER => RefundStatus::Invalid,
                    // any other owner
                    _ => RefundStatus::NotYetRefunded,
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
                RefundStatus::NotYetRefunded => {
                    to_be_refunded_uids
                        .entry(ethflow_contract.address())
                        .or_default()
                        .push(uid);
                }
            }
        }
        if !invalid_uids.is_empty() {
            // In exceptional cases, e.g. if the refunder tries to refund orders from a
            // previous contract, the order_owners could be zero
            tracing::warn!(
                "Trying to invalidate orders that weren't created in the current contract. Uids: \
                 {:?}",
                invalid_uids
            );
        }
        to_be_refunded_uids
    }

    async fn get_ethflow_data_from_db(&self, uid: &OrderUid) -> Result<EthflowOrder> {
        let mut ex = self.db.acquire().await.context("acquire")?;
        let order = read_db_order(&mut ex, uid)
            .await
            .context("read order")?
            .context("missing order")?;
        let ethflow_order = read_order(&mut ex, uid)
            .await
            .context("read ethflow order")?
            .context("missing ethflow order")?;
        Ok(order_to_ethflow_data(order, ethflow_order))
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
            let encoded_ethflow_orders: Vec<EncodedEthflowOrder> = stream::iter(futures)
                .buffer_unordered(10)
                .filter_map(|result| async {
                    match result {
                        Ok(order) => Some(order.encode()),
                        Err(err) => {
                            tracing::error!(?err, "failed to get data from db");
                            None
                        }
                    }
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
