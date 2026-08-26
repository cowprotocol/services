pub use database::order_events::{OrderEventLabel, OrderFilterReason};
use {
    crate::domain,
    anyhow::Result,
    chrono::{DateTime, Utc},
    database::{byte_array::ByteArray, order_events},
    sqlx::{Acquire, Error, PgConnection},
    tokio::time::Instant,
    tracing::instrument,
};

impl super::Postgres {
    /// Deletes events before the provided timestamp.
    #[instrument(skip_all)]
    pub async fn delete_order_events_before(&self, timestamp: DateTime<Utc>) -> Result<u64, Error> {
        order_events::delete_order_events_before(&self.pool, timestamp).await
    }
}

/// Max number of order UIDs sent to the DB per insert statement. Chunking keeps
/// individual queries bounded regardless of how many events are stored at once.
const INSERT_CHUNK_SIZE: usize = 1000;

/// Advisory-lock namespace for the deduplicating event insert, one lock per
/// label.
const DEDUP_LOCK: &str = "order_events_dedup";

/// Takes the [`DEDUP_LOCK`] of the bound label.
const DEDUP_LOCK_QUERY: &str =
    "SELECT pg_advisory_xact_lock(hashtextextended($1::text || $2::text, 0))";

pub async fn store_order_events(
    ex: &mut PgConnection,
    order_uids: impl IntoIterator<Item = domain::OrderUid>,
    label: OrderEventLabel,
    reason: Option<OrderFilterReason>,
    timestamp: DateTime<Utc>,
) {
    let start = Instant::now();

    let insert = async move {
        let mut ex = ex.begin().await?;
        // The dedup below compares against committed rows only, so writers
        // that overlap both append. Keyed per label because writers carrying
        // different labels append in either order for the same result, leaving
        // only same-label writers to serialize. The lock lives until the
        // outermost transaction commits, and only writers taking it are
        // race-free.
        sqlx::query(DEDUP_LOCK_QUERY)
            .bind(DEDUP_LOCK)
            .bind(label)
            .execute(&mut *ex)
            .await?;
        let mut order_uids = order_uids.into_iter().map(|o| ByteArray(o.0));
        let capacity = match order_uids.size_hint().1 {
            Some(hint) => std::cmp::min(hint, INSERT_CHUNK_SIZE),
            None => INSERT_CHUNK_SIZE,
        };
        let mut chunk = Vec::with_capacity(capacity);
        let mut count = 0;
        loop {
            chunk.clear();
            chunk.extend(order_uids.by_ref().take(INSERT_CHUNK_SIZE));
            if chunk.is_empty() {
                break;
            }
            count += chunk.len();
            order_events::insert_order_events(&mut ex, &chunk, timestamp, label, reason).await?;
        }
        ex.commit().await?;
        Ok::<_, Error>(count)
    };

    match insert.await {
        Ok(count) => {
            tracing::debug!(?label, count, elapsed = ?start.elapsed(), "stored order events")
        }
        Err(err) => tracing::warn!(?label, ?err, "failed to insert order events"),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::database::Postgres,
        database::{
            byte_array::ByteArray,
            order_events::{OrderEvent, OrderEventLabel},
        },
    };

    /// Unserialized writers would append the same label twice: the dedup
    /// reads committed rows only.
    #[tokio::test]
    #[ignore]
    async fn postgres_concurrent_writes_append_one_event() {
        let db = Postgres::with_defaults().await.unwrap();
        let mut ex = db.pool.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();
        let uid = ByteArray([7; 56]);
        database::order_events::insert_order_event(
            &mut ex,
            &OrderEvent {
                order_uid: uid,
                timestamp: Utc::now(),
                label: OrderEventLabel::Created,
                reason: None,
            },
        )
        .await
        .unwrap();
        ex.commit().await.unwrap();

        let write = || async {
            let mut ex = db.pool.acquire().await.unwrap();
            store_order_events(
                &mut ex,
                [domain::OrderUid(uid.0)],
                OrderEventLabel::Invalid,
                Some(OrderFilterReason::InsufficientBalance),
                Utc::now(),
            )
            .await;
        };
        tokio::join!(write(), write());

        let labels: Vec<String> = sqlx::query_scalar(
            "SELECT label::text FROM order_events WHERE order_uid = $1 ORDER BY timestamp",
        )
        .bind(uid)
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(labels, ["created", "invalid"]);
    }

    /// Only same-label writers exclude each other. While the `Invalid` lock is
    /// held, a `Filtered` write runs through and an `Invalid` write queues
    /// behind that exact lock, the one difference a per-label key makes over a
    /// table-wide one: the rows either key produces are the same.
    #[tokio::test]
    #[ignore]
    async fn postgres_dedup_lock_is_per_label() {
        /// A writer waiting on the advisory lock another session holds.
        const QUEUED_ON_HELD_LOCK: &str = r#"
SELECT EXISTS (
    SELECT 1
    FROM pg_locks held
    JOIN pg_locks waiting USING (locktype, classid, objid, objsubid)
    WHERE held.locktype = 'advisory' AND held.granted AND NOT waiting.granted
)
        "#;

        let db = Postgres::with_defaults().await.unwrap();
        let mut ex = db.pool.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();
        ex.commit().await.unwrap();
        let uid = ByteArray([7; 56]);

        let mut holder = db.pool.begin().await.unwrap();
        sqlx::query(DEDUP_LOCK_QUERY)
            .bind(DEDUP_LOCK)
            .bind(OrderEventLabel::Invalid)
            .execute(&mut *holder)
            .await
            .unwrap();

        let write = |label| {
            let pool = db.pool.clone();
            async move {
                let mut ex = pool.acquire().await.unwrap();
                store_order_events(&mut ex, [domain::OrderUid(uid.0)], label, None, Utc::now())
                    .await;
            }
        };
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            write(OrderEventLabel::Filtered),
        )
        .await
        .expect("a different label does not wait for the invalid lock");

        let blocked = tokio::spawn(write(OrderEventLabel::Invalid));
        let mut queued = false;
        for _ in 0..50 {
            queued = sqlx::query_scalar(QUEUED_ON_HELD_LOCK)
                .fetch_one(&db.pool)
                .await
                .unwrap();
            if queued {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(queued, "the same label waits on the held lock");

        holder.rollback().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), blocked)
            .await
            .expect("the write proceeds once the lock is released")
            .unwrap();
    }
}
