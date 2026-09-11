//! Replay of parked work: dead-lettered transactions are re-fetched by
//! signature and run through the same decode and flush path the stream
//! uses.

use {
    super::{Decoder, backfill::convert},
    crate::types::errors::PersistenceError,
};

/// Dead letters retried per pass, bounding the RPC load of one tick.
const DEAD_LETTER_BATCH: i64 = 100;

impl Decoder {
    /// One replay pass: retry the parked dead letters. Work that fails
    /// again stays parked for the next pass.
    pub(crate) async fn replay(&self) -> Result<(), PersistenceError> {
        for (signature, slot) in self.persistence.dead_letters(DEAD_LETTER_BATCH).await? {
            let encoded = match self.rpc.transaction(&signature).await {
                Ok(encoded) => encoded,
                // The row stays parked, the next pass retries it.
                Err(err) => {
                    tracing::warn!(?err, %signature, "failed to fetch a dead letter");
                    continue;
                }
            };
            // A payload that still fails to decode stays parked.
            let Some(events) = convert(encoded, signature)
                .and_then(|info| self.decode(info, slot, signature).ok())
            else {
                continue;
            };
            self.persistence
                .replay_events(signature, events, slot)
                .await?;
            tracing::info!(%signature, "dead letter replayed");
        }
        Ok(())
    }
}
