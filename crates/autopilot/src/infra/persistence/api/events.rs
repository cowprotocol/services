use {
    crate::{boundary, domain, infra::persistence::Persistence},
    chrono::Utc,
    tokio::time::Instant,
    tracing::Instrument,
};

impl Persistence {
    /// Inserts the given events with the current timestamp into the DB.
    /// If this function encounters an error it will only be printed. More
    /// elaborate error handling is not necessary because this is just
    /// debugging information.
    pub fn store_order_events(&self, events: Vec<(domain::OrderUid, boundary::OrderEventLabel)>) {
        let db = self.postgres.clone();
        tokio::spawn(
            async move {
                let start = Instant::now();
                match boundary::store_order_events(&db, &events, Utc::now()).await {
                    Ok(_) => {
                        tracing::debug!(elapsed=?start.elapsed(), events_count=events.len(), "stored order events");
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to insert order events");
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}
