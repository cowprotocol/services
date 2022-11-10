use anyhow::{anyhow, Result};
use contracts::CoWSwapEthFlow;
use database::{
    ethflow_orders::{
        mark_eth_orders_as_refunded, read_order, refundable_orders, EthOrderPlacement,
    },
    orders::{read_order as read_db_order, Order},
    OrderUid,
};
use ethcontract::{Account, Bytes, U256};
use futures::{stream, StreamExt};
use number_conversions::big_decimal_to_u256;
use primitive_types::H160;
use shared::ethrpc::{Web3, Web3CallBatch, MAX_BATCH_SIZE};
use sqlx::PgPool;

use crate::submitter::Submitter;

const INVALIDATED_OWNER: H160 = H160([255u8; 20]);
const MAX_NUMBER_OF_UIDS_PER_REFUND_TX: usize = 30usize;

pub type EncodedEthflowOrder = (
    H160,            // buyToken
    H160,            // receiver
    U256,            // sellAmount
    U256,            // buyAmount
    Bytes<[u8; 32]>, // appData
    U256,            // feeAmount
    u32,             // validTo
    bool,            // flags
    i64,             // quoteId
);

pub struct RefundService {
    pub db: PgPool,
    pub web3: Web3,
    pub ethflow_contract: CoWSwapEthFlow,
    pub min_validity_duration: i64,
    pub min_slippage: f64,
    pub submitter: Submitter,
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
        account: Account,
    ) -> Self {
        RefundService {
            db,
            web3: web3.clone(),
            ethflow_contract: ethflow_contract.clone(),
            min_validity_duration,
            min_slippage: min_slippage_bps as f64 / 10000f64,
            submitter: Submitter {
                web3: web3.clone(),
                ethflow_contract,
                account,
                gas_estimator: Box::new(web3),
                gas_price_of_last_submission: None,
                nonce_of_last_submission: None,
            },
        }
    }
    pub async fn try_to_refund_all_eligble_orders(&mut self) -> Result<()> {
        let refundable_order_uids = self.get_refundable_ethflow_orders_from_db().await?;

        let order_uids_per_status = self
            .identify_already_refunded_order_uids_via_web3_calls(refundable_order_uids)
            .await?;

        self.update_already_refunded_orders_in_db(order_uids_per_status.refunded)
            .await?;

        self.send_out_refunding_tx(order_uids_per_status._to_be_refunded)
            .await?;
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

    pub async fn get_refundable_ethflow_orders_from_db(&self) -> Result<Vec<EthOrderPlacement>> {
        let mut ex = self.db.acquire().await?;
        refundable_orders(
            &mut ex,
            model::time::now_in_epoch_seconds().into(),
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

    async fn send_out_refunding_tx(&mut self, uids: Vec<OrderUid>) -> Result<()> {
        if uids.is_empty() {
            return Ok(());
        }

        // only try to refund MAX_NUMBER_OF_UIDS_PER_REFUND_TX uids, in order to fit into gas limit
        let uids: Vec<OrderUid> = uids
            .into_iter()
            .take(MAX_NUMBER_OF_UIDS_PER_REFUND_TX)
            .collect();

        tracing::debug!("Trying to refunded the following uids: {:?}", uids);

        let futures = uids.iter().map(|uid| {
            let local_db = self.db.clone();
            async move {
                let mut ex = local_db.acquire().await?;
                match (
                    read_db_order(&mut ex, uid).await,
                    read_order(&mut ex, uid).await,
                ) {
                    (Ok(Some(order)), Ok(Some(ethflow_order_placement))) => {
                        Ok(Some(order_to_ethflow_data(order, ethflow_order_placement)))
                    }
                    (Err(err), _) => {
                        tracing::error!(
                            "Error while reading the order belonging to\
                                    an ethflow order: {:?}",
                            err
                        );
                        Ok(None)
                    }
                    (Ok(None), _) => {
                        tracing::error!(
                            "Could not find the order belonging to\
                                    an ethflow order"
                        );
                        Ok(None)
                    }
                    (Ok(_), Err(err)) => {
                        tracing::error!(
                            "Error while reading the ethflow order 
                                    placement: {:?}",
                            err
                        );
                        Ok(None)
                    }
                    (Ok(_), Ok(None)) => {
                        tracing::error!(
                            "Could not find the ethflow order 
                                    placement"
                        );
                        Ok(None)
                    }
                }
            }
        });
        let encoded_ethflow_orders: Vec<Result<Option<EncodedEthflowOrder>>> =
            stream::iter(futures)
                .buffer_unordered(10)
                .collect::<Vec<Result<Option<EncodedEthflowOrder>>>>()
                .await;
        let encoded_ethflow_orders: Vec<EncodedEthflowOrder> = encoded_ethflow_orders
            .into_iter()
            .flatten()
            .flatten()
            .collect();

        self.submitter.submit(uids, encoded_ethflow_orders).await?;
        Ok(())
    }
}

fn order_to_ethflow_data(
    order: Order,
    ethflow_order_placement: EthOrderPlacement,
) -> EncodedEthflowOrder {
    (
        H160(order.buy_token.0),
        H160(order.receiver.unwrap().0), // ethflow orders have always a
        // receiver. It's enforced by the contract.
        big_decimal_to_u256(&order.sell_amount).unwrap(),
        big_decimal_to_u256(&order.buy_amount).unwrap(),
        Bytes(order.app_data.0),
        big_decimal_to_u256(&order.fee_amount).unwrap(),
        ethflow_order_placement.valid_to as u32, // Unwrap can never fail, as the value
        // is not None for ethflow orders
        false, // ethflow orders are always fill or kill orders
        0i64,  // quoteId is not important for refunding and will be ignored
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::byte_array::ByteArray;
    use number_conversions::u256_to_big_decimal;

    #[test]
    fn test_order_to_ethflow_data() {
        let buy_token = ByteArray([1u8; 20]);
        let receiver = ByteArray([3u8; 20]);
        let sell_amount = U256::from_dec_str("1").unwrap();
        let buy_amount = U256::from_dec_str("2").unwrap();
        let app_data = ByteArray([3u8; 32]);
        let fee_amount = U256::from_dec_str("3").unwrap();
        let valid_to = 234u32;

        let order = Order {
            buy_token,
            receiver: Some(receiver),
            sell_amount: u256_to_big_decimal(&sell_amount),
            buy_amount: u256_to_big_decimal(&buy_amount),
            valid_to: valid_to.into(),
            app_data,
            fee_amount: u256_to_big_decimal(&fee_amount),
            ..Default::default()
        };
        let ethflow_order = EthOrderPlacement {
            valid_to: valid_to.into(),
            ..Default::default()
        };
        let expected_encoded_order = (
            H160(order.buy_token.0),
            H160(order.receiver.unwrap().0),
            big_decimal_to_u256(&order.sell_amount).unwrap(),
            big_decimal_to_u256(&order.buy_amount).unwrap(),
            Bytes(order.app_data.0),
            big_decimal_to_u256(&order.fee_amount).unwrap(),
            ethflow_order.valid_to as u32,
            false,
            0i64,
        );
        assert_eq!(
            order_to_ethflow_data(order, ethflow_order),
            expected_encoded_order
        );
    }
}
