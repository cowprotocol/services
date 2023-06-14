use {
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    model::app_id::AppDataHash,
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
}
