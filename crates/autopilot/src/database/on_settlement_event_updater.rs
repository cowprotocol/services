use {
    crate::domain,
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, settlement_observations::Observation},
    ethcontract::U256,
    model::order::OrderUid,
    number::conversions::u256_to_big_decimal,
    sqlx::PgConnection,
};

/// Executed network fee for the gas costs. This fee is solver determined.
type ExecutedFee = U256;
pub type AuctionId = i64;

#[derive(Debug, Default, Clone)]
pub struct AuctionData {
    pub gas_used: U256,
    pub effective_gas_price: U256,
    pub surplus: U256,
    pub fee: U256,
    pub order_executions: Vec<(OrderUid, ExecutedFee)>,
}

#[derive(Debug, Clone)]
pub struct SettlementUpdate {
    pub block_number: i64,
    pub log_index: i64,
    pub auction_id: AuctionId,
    /// Only set if the auction is for this environment.
    pub observation: Option<domain::settlement::Observation>,
}

impl super::Postgres {
    pub async fn update_settlement_details(
        ex: &mut PgConnection,
        settlement_update: SettlementUpdate,
    ) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_settlement_details"])
            .start_timer();

        // update settlements
        database::settlements::update_settlement_auction(
            ex,
            settlement_update.block_number,
            settlement_update.log_index,
            settlement_update.auction_id,
        )
        .await
        .context("insert_settlement_tx_info")?;

        if let Some(observation) = settlement_update.observation {
            let auction_data = AuctionData {
                gas_used: observation.gas,
                effective_gas_price: observation.effective_gas_price,
                surplus: observation.surplus.0,
                fee: observation.fee.0,
                order_executions: observation
                    .order_fees
                    .get()
                    .iter()
                    .map(|(order, fee)| ((*order).into(), fee.amount.0))
                    .collect(),
            };
            database::settlement_observations::upsert(
                ex,
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

            for (order, executed_fee) in auction_data.order_executions {
                database::order_execution::save(
                    ex,
                    &ByteArray(order.0),
                    settlement_update.auction_id,
                    settlement_update.block_number,
                    &u256_to_big_decimal(&executed_fee),
                )
                .await
                .context("save_order_executions")?;
            }
        }
        Ok(())
    }
}
