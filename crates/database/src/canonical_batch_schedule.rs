//! The canonical batch schedule the autopilot uses. The autopilot leader
//! publishes it; the orderbook reads it. At most one row: no row means
//! canonical batches are off.

use {
    sqlx::{Executor, PgConnection},
    tracing::instrument,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::FromRow)]
pub struct Schedule {
    pub interval_seconds: i32,
    pub cutoff_seconds: i32,
}

/// Stores `schedule`, replacing the previous one.
#[instrument(skip_all)]
pub async fn upsert(ex: &mut PgConnection, schedule: Schedule) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO canonical_batch_schedule (interval_seconds, cutoff_seconds, updated_at)
VALUES ($1, $2, now())
ON CONFLICT (singleton)
DO UPDATE SET
    interval_seconds = EXCLUDED.interval_seconds,
    cutoff_seconds = EXCLUDED.cutoff_seconds,
    updated_at = EXCLUDED.updated_at;
    "#;

    ex.execute(
        sqlx::query(QUERY)
            .bind(schedule.interval_seconds)
            .bind(schedule.cutoff_seconds),
    )
    .await?;
    Ok(())
}

/// Removes the schedule, which turns canonical batches off.
#[instrument(skip_all)]
pub async fn delete(ex: &mut PgConnection) -> Result<(), sqlx::Error> {
    const QUERY: &str = "DELETE FROM canonical_batch_schedule;";
    ex.execute(QUERY).await?;
    Ok(())
}

#[instrument(skip_all)]
pub async fn load(ex: &mut PgConnection) -> Result<Option<Schedule>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT interval_seconds, cutoff_seconds
FROM canonical_batch_schedule;
    "#;

    sqlx::query_as(QUERY).fetch_optional(ex).await
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_canonical_batch_schedule_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        assert_eq!(load(&mut db).await.unwrap(), None);

        let schedule = Schedule {
            interval_seconds: 60,
            cutoff_seconds: 5,
        };
        upsert(&mut db, schedule).await.unwrap();
        assert_eq!(load(&mut db).await.unwrap(), Some(schedule));

        // Publishing again replaces the row instead of adding one.
        let schedule = Schedule {
            interval_seconds: 120,
            cutoff_seconds: 0,
        };
        upsert(&mut db, schedule).await.unwrap();
        assert_eq!(load(&mut db).await.unwrap(), Some(schedule));

        delete(&mut db).await.unwrap();
        assert_eq!(load(&mut db).await.unwrap(), None);
        // Deleting when there is nothing to delete is fine.
        delete(&mut db).await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_canonical_batch_schedule_rejects_invalid_cutoff() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let result = upsert(
            &mut db,
            Schedule {
                interval_seconds: 60,
                cutoff_seconds: 60,
            },
        )
        .await;
        assert!(result.is_err());
    }
}
