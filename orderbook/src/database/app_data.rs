use super::*;
use anyhow::Result;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use model::app_data::AppDataBlob;
use primitive_types::{H160, H256};
use std::borrow::Cow;

#[async_trait::async_trait]
pub trait AppDataStoring: Send + Sync {
    async fn insert_app_data(&self, app_data: &AppDataBlob) -> Result<H256, InsertionError>;
    fn app_data<'a>(&'a self, filter: &'a AppDataFilter) -> BoxStream<'a, Result<AppDataBlob>>;
}

/// Any default value means that this field is unfiltered.
#[derive(Clone, Default)]
pub struct AppDataFilter {
    pub app_data_hash: Option<H256>,
    pub referrer: Option<H160>,
}

#[derive(Debug)]
pub enum InsertionError {
    ParsingStringError(serde_json::Error),
    DuplicatedRecord(H256),
    AnyhowError(anyhow::Error),
    DbError(sqlx::Error),
}

impl From<sqlx::Error> for InsertionError {
    fn from(err: sqlx::Error) -> Self {
        Self::DbError(err)
    }
}

impl From<serde_json::Error> for InsertionError {
    fn from(err: serde_json::Error) -> Self {
        Self::ParsingStringError(err)
    }
}

impl From<anyhow::Error> for InsertionError {
    fn from(err: anyhow::Error) -> Self {
        Self::AnyhowError(err)
    }
}

#[async_trait::async_trait]
impl AppDataStoring for Postgres {
    async fn insert_app_data(&self, app_data_str: &AppDataBlob) -> Result<H256, InsertionError> {
        let app_data = app_data_str.get_app_data()?;
        const QUERY: &str = "\
            INSERT INTO app_data (\
                app_data_hash, app_code, referrer, file_blob)\
                VALUES ($1, $2, $3, $4);";
        let app_data_hash = app_data_str.sha_hash()?;
        sqlx::query(QUERY)
            .bind(app_data_hash.clone().as_bytes())
            .bind(
                app_data
                    .app_code
                    .unwrap_or_else(|| "".to_string())
                    .as_bytes(),
            )
            .bind(
                app_data
                    .metadata
                    .unwrap_or_default()
                    .referrer
                    .unwrap_or_default()
                    .referrer
                    .as_ref(),
            )
            .bind(app_data_str.0.as_bytes())
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|err| {
                if let sqlx::Error::Database(db_err) = &err {
                    if let Some(Cow::Borrowed("23505")) = db_err.code() {
                        return InsertionError::DuplicatedRecord(app_data_hash);
                    }
                }
                InsertionError::DbError(err)
            })?;
        Ok(app_data_hash)
    }

    fn app_data<'a>(&'a self, filter: &'a AppDataFilter) -> BoxStream<'a, Result<AppDataBlob>> {
        if let Some(app_data_hash) = &filter.app_data_hash {
            const QUERY: &str = "\
            SELECT file_blob FROM app_data \
                WHERE app_data_hash = $1;";

            sqlx::query_as(QUERY)
                .bind(app_data_hash.as_bytes())
                .fetch(&self.pool)
                .err_into()
                .and_then(|row: AppDataQueryRow| async move { row.into_app_data_blob() })
                .boxed()
        } else if let Some(referrer) = &filter.referrer {
            const QUERY: &str = "\
            SELECT file_blob FROM app_data \
            WHERE referrer = $1";

            sqlx::query_as(QUERY)
                .bind(referrer.as_ref())
                .fetch(&self.pool)
                .err_into()
                .and_then(|row: AppDataQueryRow| async move { row.into_app_data_blob() })
                .boxed()
        } else {
            const QUERY: &str = "\
            SELECT file_blob FROM app_data;";

            sqlx::query_as(QUERY)
                .fetch(&self.pool)
                .err_into()
                .and_then(|row: AppDataQueryRow| async move { row.into_app_data_blob() })
                .boxed()
        }
    }
}

#[derive(sqlx::FromRow)]
struct AppDataQueryRow {
    file_blob: Vec<u8>,
}

impl AppDataQueryRow {
    fn into_app_data_blob(self) -> Result<AppDataBlob> {
        let file_blob = String::from_utf8(self.file_blob)?;
        Ok(AppDataBlob(file_blob))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use serde_json::json;
    #[tokio::test]
    #[ignore]
    async fn postgres_insert_same_order_twice_fails() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let json = json!(
        {
            "appCode": "CowSwap",
            "version": "1.0.0",
            "metadata": {
              "referrer": {
                "referrer":  "0x424a46612794dbb8000194937834250dc723ffa5",
                "version": "0.3.4",
              }
            }
        }
        );
        let app_data_blob = AppDataBlob(serde_json::to_string(&json).unwrap());
        db.insert_app_data(&app_data_blob).await.unwrap();
        match db.insert_app_data(&app_data_blob).await {
            Err(InsertionError::DuplicatedRecord(_hash)) => (),
            _ => panic!("Expecting DuplicatedRecord error"),
        };
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_app_data_roundtrip() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let filter = AppDataFilter::default();
        assert!(db.app_data(&filter).boxed().next().await.is_none());
        let json = json!(
        {
            "appCode": "CowSwap",
            "version": "1.0.0",
            "metadata": {
              "referrer": {
                "referrer":  "0x424a46612794dbb8000194937834250dc723ffa5",
                "version": "0.3.4",
              }
            }
        }
        );
        let app_data_blob = AppDataBlob(serde_json::to_string(&json).unwrap());
        db.insert_app_data(&app_data_blob).await.unwrap();
        let new_filter = AppDataFilter {
            app_data_hash: Some(app_data_blob.sha_hash().unwrap()),
            referrer: None,
        };
        assert_eq!(
            *db.app_data(&new_filter)
                .try_collect::<Vec<_>>()
                .await
                .unwrap()
                .first()
                .unwrap(),
            app_data_blob
        );
        let json = json!(
        {
            "appCode": serde_json::value::Value::Null,
            "version": "1.0.0",
            "metadata": {
              "referrer": {
                "referrer":  "0x224a46612794dbb8000194937834250dc723ffa5",
                "version": "0.3.4",
              }
            }
        }
        );
        let app_data_blob = AppDataBlob(serde_json::to_string(&json).unwrap());
        let new_filter = AppDataFilter {
            app_data_hash: None,
            referrer: Some(
                app_data_blob
                    .get_app_data()
                    .unwrap()
                    .metadata
                    .unwrap()
                    .referrer
                    .unwrap()
                    .referrer,
            ),
        };
        db.insert_app_data(&app_data_blob).await.unwrap();
        assert_eq!(
            *db.app_data(&new_filter)
                .try_collect::<Vec<_>>()
                .await
                .unwrap()
                .first()
                .unwrap(),
            app_data_blob
        );
    }
}
