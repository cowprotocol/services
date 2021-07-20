use super::*;
use crate::conversions::*;
use anyhow::Result;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use model::app_data::{AppData, MetaData};
use primitive_types::{H160, H256};
use std::borrow::Cow;

#[async_trait::async_trait]
pub trait AppDataStoring: Send + Sync {
    async fn insert_app_data(&self, app_data: &AppData) -> Result<H256, InsertionError>;
    fn app_data<'a>(&'a self, filter: &'a AppDataFilter) -> BoxStream<'a, Result<AppData>>;
}

/// Any default value means that this field is unfiltered.
#[derive(Clone, Default)]
pub struct AppDataFilter {
    pub app_data_hash: Option<H256>,
    pub referrer: Option<H160>,
}

#[derive(Debug)]
pub enum InsertionError {
    DuplicatedRecord(H256),
    AnyhowError(anyhow::Error),
    DbError(sqlx::Error),
}

impl From<sqlx::Error> for InsertionError {
    fn from(err: sqlx::Error) -> Self {
        Self::DbError(err)
    }
}

impl From<anyhow::Error> for InsertionError {
    fn from(err: anyhow::Error) -> Self {
        Self::AnyhowError(err)
    }
}

#[async_trait::async_trait]
impl AppDataStoring for Postgres {
    async fn insert_app_data(&self, app_data: &AppData) -> Result<H256, InsertionError> {
        const QUERY: &str = "\
            INSERT INTO app_data (\
                app_data_hash, version, app_code, referrer)\
                VALUES ($1, $2, $3, $4);";
        let app_data_hash = app_data.sha_hash()?;
        sqlx::query(QUERY)
            .bind(app_data_hash.clone().as_bytes())
            .bind(app_data.version.as_bytes())
            .bind(app_data.app_code.as_bytes())
            .bind(app_data.meta_data.referrer.as_ref())
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

    fn app_data<'a>(&'a self, filter: &'a AppDataFilter) -> BoxStream<'a, Result<AppData>> {
        if let Some(app_data_hash) = &filter.app_data_hash {
            const QUERY: &str = "\
            SELECT app_data_hash, version, app_code, referrer FROM app_data \
                WHERE app_data_hash = $1;";

            sqlx::query_as(QUERY)
                .bind(app_data_hash.as_bytes())
                .fetch(&self.pool)
                .err_into()
                .and_then(|row: AppDataQueryRow| async move { row.into_app_data() })
                .boxed()
        } else if let Some(referrer) = &filter.referrer {
            const QUERY: &str = "\
            SELECT app_data_hash, version, app_code, referrer FROM app_data \
            WHERE referrer = $1 \
                ORDER BY version DESC";

            sqlx::query_as(QUERY)
                .bind(referrer.as_ref())
                .fetch(&self.pool)
                .err_into()
                .and_then(|row: AppDataQueryRow| async move { row.into_app_data() })
                .boxed()
        } else {
            const QUERY: &str = "\
            SELECT app_data_hash, version, app_code, referrer FROM app_data;";

            sqlx::query_as(QUERY)
                .fetch(&self.pool)
                .err_into()
                .and_then(|row: AppDataQueryRow| async move { row.into_app_data() })
                .boxed()
        }
    }
}

#[derive(sqlx::FromRow)]
struct AppDataQueryRow {
    version: Vec<u8>,
    app_code: Vec<u8>,
    referrer: Vec<u8>,
}

impl AppDataQueryRow {
    fn into_app_data(self) -> Result<AppData> {
        let version = String::from_utf8(self.version)?;
        let app_code = String::from_utf8(self.app_code)?;
        let referrer = h160_from_vec(self.referrer)?;
        Ok(AppData {
            version,
            app_code,
            meta_data: MetaData { referrer },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_same_order_twice_fails() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let app_data = AppData {
            version: String::from("1.0.1"),
            app_code: String::from("CowSwap"),
            meta_data: MetaData {
                referrer: "0x424a46612794dbb8000194937834250dc723ffa5"
                    .parse()
                    .unwrap(),
            },
        };
        db.insert_app_data(&app_data).await.unwrap();
        match db.insert_app_data(&app_data).await {
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
        let app_data = AppData {
            version: String::from("1.0.0"),
            app_code: String::from("CowSwap"),
            meta_data: MetaData {
                referrer: "0x424a46612794dbb8000194937834250dc723ffa5"
                    .parse()
                    .unwrap(),
            },
        };
        db.insert_app_data(&app_data).await.unwrap();
        let new_filter = AppDataFilter {
            app_data_hash: Some(app_data.sha_hash().unwrap()),
            referrer: None,
        };
        assert_eq!(
            *db.app_data(&new_filter)
                .try_collect::<Vec<_>>()
                .await
                .unwrap()
                .get(0)
                .unwrap(),
            app_data
        );
        let app_data = AppData {
            version: String::from("1.0.0"),
            app_code: String::from("CowSwap"),
            meta_data: MetaData {
                referrer: "0x224a46612794dbb8000194937834250dc723ffa5"
                    .parse()
                    .unwrap(),
            },
        };
        let new_filter = AppDataFilter {
            app_data_hash: None,
            referrer: Some(app_data.meta_data.referrer),
        };
        db.insert_app_data(&app_data).await.unwrap();
        assert_eq!(
            *db.app_data(&new_filter)
                .try_collect::<Vec<_>>()
                .await
                .unwrap()
                .get(0)
                .unwrap(),
            app_data
        );
    }
}
