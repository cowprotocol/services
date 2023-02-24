use {
    anyhow::Context,
    database::settlement_observations::SettlementEvent,
    ethcontract::U256,
    number_conversions::u256_to_big_decimal,
};

impl super::Postgres {
    pub async fn update_settlement_observation(
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
        database::settlement_observations::update(
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

    pub async fn get_settlement_event_without_observation(
        &self,
        max_block_number: i64,
    ) -> Result<Option<SettlementEvent>, sqlx::Error> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_settlement_event_without_observation"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::settlement_observations::get_settlement_event_without_observation(
            &mut ex,
            max_block_number,
        )
        .await
    }
}
