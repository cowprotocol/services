use {anyhow::Context, ethcontract::U256, number_conversions::u256_to_big_decimal};

impl super::Postgres {
    pub async fn insert_settlement_observation(
        &self,
        block_number: i64,
        log_index: i64,
        gas_used: U256,
        effective_gas_price: U256,
        surplus: U256,
        fee: U256,
    ) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_settlement_observation"])
            .start_timer();

        let mut ex = self.0.acquire().await.context("acquire")?;
        database::settlement_observations::upsert(
            &mut ex,
            block_number,
            log_index,
            u256_to_big_decimal(&gas_used),
            u256_to_big_decimal(&effective_gas_price),
            u256_to_big_decimal(&surplus),
            u256_to_big_decimal(&fee),
        )
        .await
        .context("update_settlement_observation")?;

        Ok(())
    }
}
