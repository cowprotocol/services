use {
    super::Postgres,
    anyhow::Result,
    bigdecimal::BigDecimal,
    primitive_types::H160,
    std::collections::HashMap,
};

impl Postgres {
    pub async fn fetch_latest_prices(&self) -> Result<HashMap<H160, BigDecimal>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fetch_latest_prices"])
            .start_timer();

        let mut ex = self.pool.begin().await?;
        Ok(database::auction::fetch_latest_prices(&mut ex)
            .await?
            .into_iter()
            .map(|(token, price)| (H160::from(token.0), price))
            .collect::<HashMap<_, _>>())
    }
}
