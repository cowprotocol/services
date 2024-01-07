use {
    crate::{
        domain,
        infra::{self, persistence::dto},
    },
    std::collections::HashMap,
};

impl infra::Persistence {
    pub async fn read_quotes(
        &self,
        orders: impl Iterator<Item = &domain::OrderUid>,
    ) -> Result<HashMap<domain::OrderUid, domain::Quote>, Error> {
        let mut quotes = HashMap::new();
        for (id, quote) in self.postgres.read_quotes(orders).await? {
            quotes.insert(id, dto::quote::into_domain(quote)?);
        }
        Ok(quotes)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read data from database")]
    DbError(#[from] anyhow::Error),
    #[error(transparent)]
    Conversion(#[from] dto::quote::InvalidConversion),
}
