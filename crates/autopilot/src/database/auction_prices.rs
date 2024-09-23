use {
    super::Postgres,
    anyhow::Result,
    primitive_types::H160,
    shared::price_estimation::native::from_normalized_price,
    std::collections::HashMap,
};

impl Postgres {
    pub async fn fetch_latest_prices(&self) -> Result<HashMap<H160, f64>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fetch_latest_prices"])
            .start_timer();

        let mut ex = self.pool.begin().await?;
        Ok(database::auction_prices::fetch_latest_prices(&mut ex)
            .await?
            .into_iter()
            .filter_map(|auction_price| {
                Some((
                    H160::from(auction_price.token.0),
                    from_normalized_price(auction_price.price)?,
                ))
            })
            .collect::<HashMap<_, _>>())
    }
}
