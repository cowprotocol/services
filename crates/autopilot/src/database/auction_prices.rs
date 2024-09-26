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
        Ok(database::auction_prices::fetch_latest_prices(&mut ex)
            .await?
            .into_iter()
            .map(|auction_price| (H160::from(auction_price.token.0), auction_price.price))
            .collect::<HashMap<_, _>>())
    }
}
