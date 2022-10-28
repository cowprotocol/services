use anyhow::Result;
use contracts::CoWSwapEthFlow;
use database::{
    ethflow_orders::{mark_eth_orders_as_refunded, refundable_orders},
    OrderUid,
};
use lazy_static::lazy_static;
use primitive_types::H160;
use shared::{transport::MAX_BATCH_SIZE, Web3, Web3CallBatch};
use sqlx::PgPool;
use std::time::SystemTime;

// The following address is used in the ethflow contract to mark invalidated orders
lazy_static! {
    static ref INVALIDATED_OWNER: H160 = H160([255u8; 20]);
}

pub struct RefundService {
    pub db: PgPool,
    pub web3: Web3,
    pub ethflow_contract: CoWSwapEthFlow,
    pub min_validity_duration: i64,
    pub min_slippage: f64,
}

type SplittedOrderUids = (Vec<(OrderUid, bool)>, Vec<(OrderUid, bool)>);

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
        // Look for all refundable orders by calling the database
        let unix_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut ex = self.db.acquire().await?;
        let refundable_order_uids = refundable_orders(
            &mut ex,
            unix_timestamp,
            self.min_validity_duration,
            self.min_slippage,
        )
        .await?;

        // Identify orderUids that are already refunded
        // by making a batched web3 call.

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
                        Err(err) => return Err(err),
                    };
                    let is_refunded: bool = order_owner == Some(*INVALIDATED_OWNER);
                    Ok((eth_order_placement.uid, is_refunded))
                }
            })
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;
        let uid_with_latest_refundablility = futures::future::try_join_all(futures).await?;

        let (refunded, _to_be_refunded): SplittedOrderUids = uid_with_latest_refundablility
            .into_iter()
            .partition(|(_, is_refunded)| *is_refunded);

        // Update refunded ethflow_orders in the database
        let refunded_uids: Vec<OrderUid> = refunded.into_iter().map(|(uid, _)| uid).collect();
        mark_eth_orders_as_refunded(&mut ex, refunded_uids.as_slice()).await?;

        // Send out tx to deleteOrders, and wait for 5 block for the tx
        // to be mined
        // todo()

        Ok(())
    }
}
