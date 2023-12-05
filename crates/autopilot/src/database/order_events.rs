pub use database::order_events::OrderEventLabel;
use {
    anyhow::{Context, Result},
    async_trait::async_trait,
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        order_events::{self, OrderEvent},
    },
    model::order::OrderUid,
    sqlx::{Error, PgConnection},
};

impl super::Postgres {
    /// Inserts the given events with the current timestamp into the DB.
    /// If this function encounters an error it will only be printed. More
    /// elaborate error handling is not necessary because this is just
    /// debugging information.
    pub async fn store_order_events(&self, events: &[(OrderUid, OrderEventLabel)]) {
        if let Err(err) = store_all_order_events(self, events, Utc::now()).await {
            tracing::warn!(?err, "failed to insert order events");
        }
    }

    /// Inserts the given events with the current timestamp into the DB for the
    /// market orders only.
    pub async fn store_market_order_events(&self, events: &[(OrderUid, OrderEventLabel)]) {
        if let Err(err) = store_market_order_events(self, events, Utc::now()).await {
            tracing::warn!(?err, "failed to insert market order events");
        }
    }

    /// Deletes events before the provided timestamp.
    pub async fn delete_order_events_before(&self, timestamp: DateTime<Utc>) -> Result<u64, Error> {
        order_events::delete_order_events_before(&self.0, timestamp).await
    }
}

#[async_trait]
pub trait OrderEventPersister {
    async fn insert_order_event(
        &self,
        conn: &mut PgConnection,
        event: &OrderEvent,
    ) -> Result<(), Error>;
}

struct RegularOrderEventPersister;

#[async_trait]
impl OrderEventPersister for RegularOrderEventPersister {
    async fn insert_order_event(
        &self,
        conn: &mut PgConnection,
        event: &OrderEvent,
    ) -> Result<(), Error> {
        order_events::insert_order_event(conn, event).await
    }
}

struct MarketOrderEventPersister;

#[async_trait]
impl OrderEventPersister for MarketOrderEventPersister {
    async fn insert_order_event(
        &self,
        conn: &mut PgConnection,
        event: &OrderEvent,
    ) -> Result<(), Error> {
        order_events::insert_market_order_event(conn, event).await
    }
}

async fn store_order_events_generic(
    db: &super::Postgres,
    events: &[(OrderUid, OrderEventLabel)],
    timestamp: DateTime<Utc>,
    persister: &impl OrderEventPersister,
) -> Result<()> {
    let mut ex = db.0.begin().await.context("begin transaction")?;
    for (uid, label) in events {
        let event = OrderEvent {
            order_uid: ByteArray(uid.0),
            timestamp,
            label: *label,
        };

        persister.insert_order_event(&mut ex, &event).await?;
    }
    ex.commit().await?;
    Ok(())
}

async fn store_all_order_events(
    db: &super::Postgres,
    events: &[(OrderUid, OrderEventLabel)],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    store_order_events_generic(db, events, timestamp, &RegularOrderEventPersister).await
}

async fn store_market_order_events(
    db: &super::Postgres,
    events: &[(OrderUid, OrderEventLabel)],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    store_order_events_generic(db, events, timestamp, &MarketOrderEventPersister).await
}
