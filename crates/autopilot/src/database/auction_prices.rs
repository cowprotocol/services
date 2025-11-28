use {
    super::Postgres,
    alloy::primitives::Address,
    anyhow::Result,
    bigdecimal::BigDecimal,
    std::collections::HashMap,
};

impl Postgres {
    pub async fn fetch_latest_prices(&self) -> Result<HashMap<Address, BigDecimal>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fetch_latest_prices"])
            .start_timer();

        let mut ex = self.pool.begin().await?;
        Ok(database::auction_prices::fetch_latest_prices(&mut ex)
            .await?
            .into_iter()
            .map(|auction_price| (Address::new(auction_price.token.0), auction_price.price))
            .collect::<HashMap<_, _>>())
    }
}
