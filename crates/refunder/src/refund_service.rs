use anyhow::{anyhow, Result};
use contracts::CoWSwapEthFlow;
use database::{
    ethflow_orders::{mark_eth_orders_as_refunded, refundable_orders, EthOrderPlacement},
    OrderUid,
};
use primitive_types::H160;
use shared::ethrpc::{Web3, Web3CallBatch, MAX_BATCH_SIZE};
use sqlx::PgPool;
use std::time::SystemTime;

const INVALIDATED_OWNER: H160 = H160([255u8; 20]);

pub struct RefundService {
    pub db: PgPool,
    pub web3: Web3,
    pub ethflow_contract: CoWSwapEthFlow,
    pub min_validity_duration: i64,
    pub min_slippage: f64,
}

struct SplittedOrderUids {
    refunded: Vec<OrderUid>,
    _to_be_refunded: Vec<OrderUid>,
}

impl RefundService {
    pub fn new(
        db: PgPool,
        web3: Web3,
        ethflow_contract: CoWSwapEthFlow,
        min_validity_duration: i64,
        min_slippage_bps: u64,
    ) -> Self {
        RefundService {
            db,
            web3,
            ethflow_contract,
            min_validity_duration,
            min_slippage: min_slippage_bps as f64 / 10000f64,
        }
    }
    pub async fn try_to_refund_all_eligble_orders(&self) -> Result<()> {
        let refundable_order_uids = self.get_refundable_ethflow_orders_from_db().await?;

        let order_uids_per_status = self
            .identify_already_refunded_order_uids_via_web3_calls(refundable_order_uids)
            .await?;

        self.update_already_refunded_orders_in_db(order_uids_per_status.refunded)
            .await?;

        // Send out tx to deleteOrders, and wait for 5 block for the tx
        // to be mined
        // todo()

        Ok(())
    }

    async fn update_already_refunded_orders_in_db(
        &self,
        refunded_uids: Vec<OrderUid>,
    ) -> Result<()> {
        let mut transaction = self.db.begin().await?;
        mark_eth_orders_as_refunded(&mut transaction, refunded_uids.as_slice())
            .await
            .map_err(|err| {
                anyhow!(
                    "Error while retrieving updating the already refunded orders:{:?}",
                    err
                )
            })?;

        Ok(transaction.commit().await?)
    }

    async fn get_refundable_ethflow_orders_from_db(&self) -> Result<Vec<EthOrderPlacement>> {
        let unix_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut ex = self.db.acquire().await?;
        refundable_orders(
            &mut ex,
            unix_timestamp,
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

    async fn identify_already_refunded_order_uids_via_web3_calls(
        &self,
        refundable_order_uids: Vec<EthOrderPlacement>,
    ) -> Result<SplittedOrderUids> {
        let mut batch = Web3CallBatch::new(self.web3.transport().clone());
        let futures = refundable_order_uids
            .iter()
            .map(|eth_order_placement| {
                let order_hash: [u8; 32] = eth_order_placement.uid.0[0..32]
                    .try_into()
                    .expect("order_uid slice with incorrect length");
                let order = self
                    .ethflow_contract
                    .orders(ethcontract::tokens::Bytes(order_hash))
                    .batch_call(&mut batch);
                async move {
                    let order_owner = match order.await {
                        Ok(order) => Some(order.0),
                        Err(err) => {
                            tracing::error!(
                                "Error while getting the current\
                                            onchain status of orderhash {:?}, {:?}",
                                order_hash,
                                err
                            );
                            return None;
                        }
                    };
                    let is_refunded: bool = order_owner == Some(INVALIDATED_OWNER);
                    Some((eth_order_placement.uid, is_refunded))
                }
            })
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;
        let uid_with_latest_refundablility = futures::future::join_all(futures).await;
        type TupleWithRefundStatus = (Vec<(OrderUid, bool)>, Vec<(OrderUid, bool)>);
        let (refunded_uids, to_be_refunded_uids): TupleWithRefundStatus =
            uid_with_latest_refundablility
                .into_iter()
                .flatten()
                .partition(|(_, is_refunded)| *is_refunded);
        let refunded_uids: Vec<OrderUid> = refunded_uids.into_iter().map(|(uid, _)| uid).collect();
        let to_be_refunded_uids: Vec<OrderUid> = to_be_refunded_uids
            .into_iter()
            .map(|(uid, _)| uid)
            .collect();
        let result = SplittedOrderUids {
            refunded: refunded_uids,
            _to_be_refunded: to_be_refunded_uids,
        };
        Ok(result)
    }
}
