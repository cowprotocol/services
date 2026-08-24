//! Auction provider backed by the indexer-written tables.

use {
    crate::{domain::cycle::SolanaCycle, infra::db, run_loop::AuctionProvider},
    async_trait::async_trait,
    sqlx::PgPool,
    std::{
        sync::atomic::{AtomicI64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// Cuts auctions from the open orders the indexer persisted.
pub struct DbAuctionProvider {
    pool: PgPool,
    /// Last allocated auction id. Ids are unix seconds, bumped past the
    /// previous allocation when cycles land within the same second. Unique
    /// only per process: no table allocates auction ids.
    /// TODO: allocate from the auctions table sequence once competition
    /// persistence lands, like the EVM `auctions.id` bigserial.
    last_id: AtomicI64,
}

impl DbAuctionProvider {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            last_id: AtomicI64::new(0),
        }
    }

    /// Allocates the next auction id: the current unix second, or one past
    /// the previous id when several cycles land within the same second, so
    /// ids strictly increase within the process.
    fn next_id(&self, now: i64) -> i64 {
        let prev = self
            .last_id
            .update(Ordering::Relaxed, Ordering::Relaxed, |prev| {
                now.max(prev + 1)
            });
        now.max(prev + 1)
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after the unix epoch")
        .as_secs()
        .try_into()
        .expect("unix seconds fit i64")
}

#[async_trait]
impl AuctionProvider<SolanaCycle> for DbAuctionProvider {
    /// The order data is push-fed by the indexer, there is no cache to
    /// refresh.
    async fn sync_to_tip(&self, _tip: &u64) -> anyhow::Result<()> {
        Ok(())
    }

    async fn cut_auction(&self, _tip: &u64) -> Option<crate::domain::auction::Auction> {
        let now = now_unix();
        let auction = db::cut(&self.pool, self.next_id(now), now)
            .await
            .map_err(|err| tracing::warn!(?err, "failed to cut the auction"))
            .ok()?;
        (!auction.orders.is_empty()).then_some(auction)
    }
}
