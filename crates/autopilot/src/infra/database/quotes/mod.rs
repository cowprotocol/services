use {super::Database, crate::domain, model::order::OrderUid, std::collections::HashMap};

pub mod dto;

impl Database {
    pub async fn read_quotes(
        &self,
        orders: impl Iterator<Item = &OrderUid>,
    ) -> Result<HashMap<OrderUid, domain::Quote>, Error> {
        let mut quotes = HashMap::new();
        for (id, quote) in self.db.read_quotes(orders).await? {
            quotes.insert(id, dto::into_domain(quote)?);
        }
        Ok(quotes)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read data from database")]
    DbError(#[from] anyhow::Error),
    #[error(transparent)]
    Conversion(#[from] dto::InvalidConversion),
}
