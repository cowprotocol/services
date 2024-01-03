use {
    crate::{boundary::OrderUid, database::Postgres},
    anyhow::Result,
    database::byte_array::ByteArray,
    std::collections::HashMap,
};

pub mod dto;

impl Postgres {
    /// Get quotes for all orders in the auction.
    ///
    /// Doens't guarantee that all orders have quotes.
    pub async fn read_quotes(
        &self,
        orders: impl Iterator<Item = &OrderUid>,
    ) -> Result<HashMap<OrderUid, database::orders::Quote>> {
        let mut ex = self.pool.acquire().await?;
        let mut quotes = HashMap::new();
        for order in orders {
            let order_uid = ByteArray(order.0);
            if let Some(quote) = database::orders::read_quote(&mut ex, &order_uid).await? {
                quotes.insert(*order, quote);
            }
        }
        Ok(quotes)
    }
}
