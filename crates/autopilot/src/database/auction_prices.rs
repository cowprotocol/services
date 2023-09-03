use {
    anyhow::Context,
    database::auction::AuctionId,
    number::conversions::big_decimal_to_u256,
    primitive_types::{H160, U256},
    sqlx::PgConnection,
    std::collections::BTreeMap,
};

impl super::Postgres {
    pub async fn get_auction_prices(
        ex: &mut PgConnection,
        auction_id: AuctionId,
    ) -> anyhow::Result<BTreeMap<H160, U256>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_auction_prices"])
            .start_timer();

        let prices = database::auction_prices::fetch(ex, auction_id)
            .await
            .with_context(|| format!("get_auction_prices for auction {auction_id}"))?
            .into_iter()
            .map(|p| (H160(p.token.0), big_decimal_to_u256(&p.price).unwrap()))
            .collect();
        Ok(prices)
    }
}
