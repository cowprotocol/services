use {
    anyhow::{Context, Result},
    shared::canonical_batch::Schedule,
    std::num::NonZeroU32,
};

impl super::Postgres {
    /// Loads the canonical batch schedule the autopilot published. `None`
    /// means canonical batches are off.
    pub async fn canonical_batch_schedule(&self) -> Result<Option<Schedule>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["canonical_batch_schedule"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let Some(row) = database::canonical_batch_schedule::load(&mut ex).await? else {
            return Ok(None);
        };
        let interval = u32::try_from(row.interval_seconds)
            .ok()
            .and_then(NonZeroU32::new)
            .context("published canonical batch interval is not positive")?;
        let cutoff = u32::try_from(row.cutoff_seconds)
            .context("published canonical batch cutoff is negative")?;
        let schedule = Schedule::new(interval, cutoff)
            .context("published canonical batch cutoff is not shorter than the interval")?;
        Ok(Some(schedule))
    }
}
