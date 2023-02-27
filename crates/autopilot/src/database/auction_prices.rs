use {
    anyhow::Context,
    database::{auction::AuctionId, auction_prices::AuctionPrice, byte_array::ByteArray},
    number_conversions::{big_decimal_to_u256, u256_to_big_decimal},
    primitive_types::{H160, U256},
    std::collections::BTreeMap,
};

impl super::Postgres {
    /// Insert external prices for an auction, provided by the autopilot.
    pub async fn insert_auction_prices(
        &self,
        auction_id: AuctionId,
        prices: &BTreeMap<H160, U256>,
    ) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["insert_auction_prices"])
            .start_timer();

        let mut ex = self.0.begin().await?;

        database::auction_prices::delete(&mut ex, auction_id)
            .await
            .context("delete_auction_prices")?;
        database::auction_prices::insert(
            &mut ex,
            prices
                .iter()
                .map(|(token, price)| AuctionPrice {
                    auction_id,
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

    pub async fn get_auction_prices(
        &self,
        auction_id: AuctionId,
    ) -> anyhow::Result<BTreeMap<H160, U256>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_auction_prices"])
            .start_timer();

        let mut ex = self.0.acquire().await.context("acquire")?;
        let prices = database::auction_prices::fetch(&mut ex, auction_id)
            .await
            .context("get_auction_prices")?;
        let prices = prices
            .into_iter()
            .map(|p| (H160(p.token.0), big_decimal_to_u256(&p.price).unwrap()))
            .collect();
        Ok(prices)
    }
}
