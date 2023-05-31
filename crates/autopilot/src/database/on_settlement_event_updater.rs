use {
    anyhow::Context,
    database::{byte_array::ByteArray, settlement_observations::Observation},
    ethcontract::{H160, U256},
    model::order::OrderUid,
    number_conversions::u256_to_big_decimal,
};

#[derive(Debug, Default, Clone)]
pub struct AuctionData {
    pub auction_id: i64,
    pub gas_used: U256,
    pub effective_gas_price: U256,
    pub surplus: U256,
    pub fee: U256,
    // pairs <order id, fee> for partial limit orders
    pub order_executions: Vec<(OrderUid, U256)>,
}

#[derive(Debug, Default, Clone)]
pub struct SettlementUpdate {
    pub block_number: i64,
    pub log_index: i64,
    pub tx_from: H160,
    pub tx_nonce: i64,
    pub auction_data: Option<AuctionData>,
}

impl super::Postgres {
    pub async fn update_settlement_details(
        &self,
        settlement_update: SettlementUpdate,
    ) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_settlement_details"])
            .start_timer();

        let mut ex = self.0.begin().await?;

        // update settlements
        database::auction_transaction::insert_settlement_tx_info(
            &mut ex,
            settlement_update.block_number,
            settlement_update.log_index,
            &ByteArray(settlement_update.tx_from.0),
            settlement_update.tx_nonce,
        )
        .await
        .context("insert_settlement_tx_info")?;

        if let Some(auction_data) = settlement_update.auction_data {
            database::settlement_observations::insert(
                &mut ex,
                Observation {
                    block_number: settlement_update.block_number,
                    log_index: settlement_update.log_index,
                    gas_used: u256_to_big_decimal(&auction_data.gas_used),
                    effective_gas_price: u256_to_big_decimal(&auction_data.effective_gas_price),
                    surplus: u256_to_big_decimal(&auction_data.surplus),
                    fee: u256_to_big_decimal(&auction_data.fee),
                },
            )
            .await
            .context("insert_settlement_observations")?;

            // update order executions for partial limit orders
            // partial limit orders are a special kind of orders for which the surplus_fee
            // is calculated AFTER the settlement is settled on chain.
            for order_execution in auction_data.order_executions {
                database::order_execution::update_surplus_fee(
                    &mut ex,
                    &ByteArray(order_execution.0 .0), // order uid
                    auction_data.auction_id,
                    Some(order_execution.1) // order fee
                        .as_ref()
                        .map(u256_to_big_decimal)
                        .as_ref(),
                )
                .await
                .context("insert_missing_order_executions")?;
            }
        }

        ex.commit().await?;
        Ok(())
    }
}
