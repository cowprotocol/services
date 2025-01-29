use {
    super::ethflow_order::{order_to_ethflow_data, EncodedEthflowOrder, EthflowOrder},
    crate::submitter::Submitter,
    anyhow::{anyhow, Context, Result},
    contracts::CoWSwapEthFlow,
    database::{
        ethflow_orders::{read_order, refundable_orders, EthOrderPlacement},
        orders::read_order as read_db_order,
        OrderUid,
    },
    ethcontract::{Account, H160, H256},
    ethrpc::{
        block_stream::timestamp_of_current_block_in_seconds,
        Web3,
        Web3CallBatch,
        MAX_BATCH_SIZE,
    },
    futures::{stream, StreamExt},
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
    ) -> Vec<(OrderUid, Vec<CoWSwapEthFlowAddress>)> {
        let mut batch = Web3CallBatch::new(self.web3.transport().clone());
        let futures = refundable_order_uids
            .iter()
            .map(|eth_order_placement| {
                let order_hash: [u8; 32] = eth_order_placement.uid.0[0..32]
                    .try_into()
                    .expect("order_uid slice with incorrect length");

                let contract_calls = self
                    .ethflow_contracts
                    .iter()
                    .map(|contract| {
                        contract
                            .orders(ethcontract::tokens::Bytes(order_hash))
                            .batch_call(&mut batch)
                    })
                    .collect::<Vec<_>>();

                async move {
                    // Theoretically, multiple orders with the same order_hash can exist in multiple
                    // contracts at the same time. That's why we need to return a list of contracts
                    // from which the order should be refunded.
                    let mut ethflow_contracts = vec![];
                    for (index, call) in contract_calls.into_iter().enumerate() {
                        match call.await {
                            Ok((owner, _)) => {
                                if owner != INVALIDATED_OWNER && owner != NO_OWNER {
                                    // order has a valid owner, so we should refund it
                                    ethflow_contracts.push(self.ethflow_contracts[index].address());
                                }
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Error while getting the currentonchain status of orderhash \
                                     {:?}, {:?}",
                                    H256(order_hash),
                                    err
                                );
                            }
                        }
                    }
                    (eth_order_placement.uid, ethflow_contracts)
                }
            })
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;
        futures::future::join_all(futures).await
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
        uids: Vec<(OrderUid, Vec<CoWSwapEthFlowAddress>)>,
    ) -> Result<()> {
        if uids.is_empty() {
            return Ok(());
        }

        // Go over all order uids and group them by ethflow contract address
        let mut uids_by_contract: HashMap<CoWSwapEthFlowAddress, Vec<OrderUid>> = HashMap::new();
        for (uid, contract) in uids.iter() {
            for c in contract.iter() {
                uids_by_contract.entry(*c).or_default().push(*uid);
            }
        }

        // For each ethflow contract, issue a separate tx to refund
        for (contract, uids) in uids_by_contract.into_iter() {
            // only try to refund MAX_NUMBER_OF_UIDS_PER_REFUND_TX uids, in order to fit
            // into gas limit
            let uids = uids
                .into_iter()
                .take(MAX_NUMBER_OF_UIDS_PER_REFUND_TX)
                .collect::<Vec<_>>();

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
            let ethflow_contract = self
                .ethflow_contracts
                .iter()
                .find(|c| c.address() == contract)
                .ok_or_else(|| anyhow!("Could not find contract with address: {:?}", contract))?;
            self.submitter
                .submit(uids, encoded_ethflow_orders, ethflow_contract)
                .await?;
        }

        Ok(())
    }
}
