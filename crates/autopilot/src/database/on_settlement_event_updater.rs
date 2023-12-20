use {
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, settlement_observations::Observation},
    ethcontract::{H160, U256},
    model::order::OrderUid,
    number::conversions::u256_to_big_decimal,
    sqlx::PgConnection,
};

/// Executed network fee for the gas costs. This fee is solver determined.
type ExecutedFee = U256;

#[derive(Debug, Default, Clone)]
pub struct AuctionData {
    pub auction_id: AuctionId,
    pub gas_used: U256,
    pub effective_gas_price: U256,
    pub surplus: U256,
    pub fee: U256,
    pub order_executions: Vec<(OrderUid, ExecutedFee)>,
}

#[derive(Debug, Clone)]
pub enum AuctionId {
    /// We were able to recover this ID from our DB which means that it was
    /// submitted by us when the protocol was not yet using colocated
    /// drivers. This ID can therefore be trusted.
    Centralized(i64),
    /// This ID had to be recovered from the calldata of a settlement call. That
    /// means it was submitted by a colocated driver. Because these drivers
    /// could submit a solution at any time and with wrong or malicious IDs
    /// this can not be trusted. For DB updates that modify existing
    /// data based on these IDs we have to ensure they can only be executed once
    /// (the first time we see this ID). That is required to prevent
    /// malicious drivers from overwriting data for already settled
    /// auctions.
    Colocated(i64),
}

impl AuctionId {
    /// Returns the underlying `auction_id` assuming the caller verified that
    /// the next DB update will not run into problems with this ID.
    pub fn assume_verified(&self) -> i64 {
        match &self {
            Self::Centralized(id) => *id,
            Self::Colocated(id) => *id,
        }
    }
}

impl Default for AuctionId {
    fn default() -> Self {
        Self::Colocated(0)
    }
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
        ex: &mut PgConnection,
        settlement_update: SettlementUpdate,
    ) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_settlement_details"])
            .start_timer();

        // update settlements
        database::auction_transaction::insert_settlement_tx_info(
            ex,
            settlement_update.block_number,
            settlement_update.log_index,
            &ByteArray(settlement_update.tx_from.0),
            settlement_update.tx_nonce,
        )
        .await
        .context("insert_settlement_tx_info")?;

        if let Some(auction_data) = settlement_update.auction_data {
            // Link the `auction_id` to the settlement tx. This is needed for
            // colocated solutions and is a no-op for centralized
            // solutions.
            let insert_succesful = database::auction_transaction::try_insert_auction_transaction(
                ex,
                auction_data.auction_id.assume_verified(),
                &ByteArray(settlement_update.tx_from.0),
                settlement_update.tx_nonce,
            )
            .await
            .context("failed to insert auction_transaction")?;

            // in case of deep reindexing we might already have the observation, so just
            // overwrite it
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

            if insert_succesful || matches!(auction_data.auction_id, AuctionId::Centralized(_)) {
                for (order, executed_fee) in auction_data.order_executions {
                    database::order_execution::save(
                        ex,
                        &ByteArray(order.0),
                        auction_data.auction_id.assume_verified(),
                        &u256_to_big_decimal(&executed_fee),
                    )
                    .await
                    .context("save_order_executions")?;
                }
            }
        }
        Ok(())
    }
}
