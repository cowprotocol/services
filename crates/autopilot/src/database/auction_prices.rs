use {
    anyhow::Context,
    database::{auction::AuctionId, byte_array::ByteArray},
    number_conversions::{big_decimal_to_u256, u256_to_big_decimal},
    primitive_types::{H160, U256},
    std::collections::BTreeMap,
};

impl super::Postgres {
    pub async fn insert_auction_prices(
        &self,
        auction_id: AuctionId,
        prices: &BTreeMap<H160, U256>,
    ) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["insert_auction_prices"])
            .start_timer();

        let mut ex = self.0.acquire().await.context("acquire")?;
        database::auction_prices::upsert(
            &mut ex,
            auction_id,
            prices.keys().map(|p| ByteArray(p.0)).collect(),
            prices.values().map(u256_to_big_decimal).collect(),
        )
        .await
        .context("insert_auction_prices")?;

        Ok(())
    }

    pub async fn fetch_auction_prices(
        &self,
        auction_id: AuctionId,
    ) -> anyhow::Result<BTreeMap<H160, U256>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fetch_auction_prices"])
            .start_timer();

        let mut ex = self.0.acquire().await.context("acquire")?;
        let prices = database::auction_prices::fetch(&mut ex, auction_id)
            .await
            .context("fetch_auction_prices")?;
        prices
            .map(|p| {
                p.tokens
                    .into_iter()
                    .zip(p.prices.into_iter())
                    .map(|(token, price)| (H160(token.0), big_decimal_to_u256(&price).unwrap()))
                    .collect()
            })
            .context("fetch_auction_prices")
    }
}
