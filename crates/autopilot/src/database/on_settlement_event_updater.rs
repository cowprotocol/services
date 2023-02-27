use {
    anyhow::Context,
    database::{
        auction_prices::AuctionPrice,
        byte_array::ByteArray,
        settlement_observations::Observation,
    },
    ethcontract::{H160, U256},
    number_conversions::u256_to_big_decimal,
    std::collections::BTreeMap,
};

#[derive(Debug, Clone)]
pub struct SettlementUpdate {
    pub block_number: i64,
    pub log_index: i64,
    pub auction_id: i64,
    pub tx_from: H160,
    pub tx_nonce: i64,
    pub gas_used: U256,
    pub effective_gas_price: U256,
    pub surplus: U256,
    pub fee: U256,
    // external prices of tokens used in the settlement, that we want to keep in database
    pub prices: BTreeMap<H160, U256>,
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

        // update settlement_observations
        database::settlement_observations::insert(
            &mut ex,
            Observation {
                block_number: settlement_update.block_number,
                log_index: settlement_update.log_index,
                gas_used: u256_to_big_decimal(&settlement_update.gas_used),
                effective_gas_price: u256_to_big_decimal(&settlement_update.effective_gas_price),
                surplus: u256_to_big_decimal(&settlement_update.surplus),
                fee: u256_to_big_decimal(&settlement_update.fee),
            },
        )
        .await
        .context("insert_settlement_observations")?;

        // Update auction_prices
        // We first delete all external prices for the auction and then insert the
        // external prices we want to keep (the ones used in the settlement)
        // Note that we could instead just delete the external prices not used in the
        // settlement
        database::auction_prices::delete(&mut ex, settlement_update.auction_id)
            .await
            .context("delete_auction_prices")?;
        database::auction_prices::insert(
            &mut ex,
            settlement_update
                .prices
                .iter()
                .map(|(token, price)| AuctionPrice {
                    auction_id: settlement_update.auction_id,
                    token: ByteArray(token.0),
                    price: u256_to_big_decimal(price),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("insert_auction_prices")?;

        ex.commit().await?;
        Ok(())
    }
}
