use {crate::database::Postgres, anyhow::Context, sqlx};

/// A wrapper around a database transaction.
#[must_use = "transactions are only executed if you `.commit` them"]
pub struct Transaction {
    pub(super) inner: sqlx::Transaction<'static, sqlx::Postgres>,
}

impl Transaction {
    pub async fn begin(postgres: &Postgres) -> Result<Self, super::Error> {
        Ok(Self {
            inner: postgres
                .pool
                .begin()
                .await
                .context("transaction begin")
                .map_err(super::Error::DbError)?,
        })
    }

    pub async fn commit(self) -> Result<(), super::Error> {
        self.inner
            .commit()
            .await
            .context("transaction commit")
            .map_err(super::Error::DbError)
    }
}
