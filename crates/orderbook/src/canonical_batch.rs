//! The canonical batch schedule as the orderbook sees it: the one the
//! autopilot leader published to the database.

use {
    crate::database::Postgres,
    anyhow::Result,
    shared::canonical_batch::{Schedule, ScheduleReading},
    std::{
        sync::Mutex,
        time::{Duration, Instant},
    },
};

/// Reads the published schedule and keeps it for `max_age`, so placing
/// orders and polling the schedule endpoint don't query the database each
/// time. The schedule only changes when a new autopilot version starts
/// leading.
pub struct ScheduleCache {
    database: Postgres,
    max_age: Duration,
    cached: Mutex<Option<(Instant, Option<Schedule>)>>,
}

impl ScheduleCache {
    pub fn new(database: Postgres, max_age: Duration) -> Self {
        Self {
            database,
            max_age,
            cached: Mutex::new(None),
        }
    }
}

/// Introduced to facilitate unit testing
#[async_trait::async_trait]
impl ScheduleReading for ScheduleCache {
    async fn schedule(&self) -> Result<Option<Schedule>> {
        let cached = *self.cached.lock().unwrap();
        if let Some((fetched_at, schedule)) = cached
            && fetched_at.elapsed() < self.max_age
        {
            return Ok(schedule);
        }
        let schedule = self.database.canonical_batch_schedule().await?;
        *self.cached.lock().unwrap() = Some((Instant::now(), schedule));
        Ok(schedule)
    }
}
