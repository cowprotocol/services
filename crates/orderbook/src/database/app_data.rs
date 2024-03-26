use {
    anyhow::{Context, Result},
    app_data::AppDataHash,
    database::byte_array::ByteArray,
    std::string::FromUtf8Error,
};

impl super::Postgres {
    pub async fn get_full_app_data(
        &self,
        contract_app_data: &AppDataHash,
    ) -> Result<Option<String>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_full_app_data"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let full_app_data =
            match database::app_data::fetch(&mut ex, &ByteArray(contract_app_data.0)).await? {
                Some(inner) => inner,
                None => return Ok(None),
            };
        let full_app_data = String::from_utf8(full_app_data).context("app data is not utf-8")?;
        Ok(Some(full_app_data))
    }

    pub async fn insert_full_app_data(
        &self,
        contract_app_data: &AppDataHash,
        full_app_data: &str,
    ) -> Result<(), InsertError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["insert_full_app_data"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        if let Some(existing) = database::app_data::insert(
            &mut ex,
            &ByteArray(contract_app_data.0),
            full_app_data.as_bytes(),
        )
        .await?
        {
            return if existing != full_app_data.as_bytes() {
                let existing = String::from_utf8(existing)?;
                Err(InsertError::Mismatch(existing))
            } else {
                Err(InsertError::Duplicate)
            };
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum InsertError {
    Duplicate,
    Mismatch(String),
    Other(anyhow::Error),
}

impl From<sqlx::Error> for InsertError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<FromUtf8Error> for InsertError {
    fn from(err: FromUtf8Error) -> Self {
        Self::Other(err.into())
    }
}
