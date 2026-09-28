//! Per-auction cache of an auction's [`BuyTokenAccounts`].

use {
    super::{auction::Id, order_uid::OrderUid},
    moka::future::Cache,
    std::{collections::HashSet, future::Future, sync::Arc, time::Duration},
};

/// How long a resolution stays cached. An auction is solved by every engine
/// within one deadline, so this only has to outlive that.
const CACHE_TTL: Duration = Duration::from_secs(60);

/// How an auction's buy token accounts stand on chain.
#[derive(Debug, Default)]
pub struct BuyTokenAccounts {
    /// Orders whose account the settlement creates before it pays out, so the
    /// engine can price its rent in.
    pub missing: HashSet<OrderUid>,
    /// Orders whose account can neither receive the payout nor be created by
    /// the settlement: their settlement would revert at `FinalizeSettle`.
    pub unreceivable: HashSet<OrderUid>,
}

/// Each auction's [`BuyTokenAccounts`], shared by every solver engine this
/// driver hosts.
///
/// The autopilot sends each engine the same auction, so the resolution it
/// holds runs once per auction instead of once per engine.
#[derive(Clone)]
pub struct BuyTokenAccountCache(Cache<Id, Arc<BuyTokenAccounts>>);

impl Default for BuyTokenAccountCache {
    fn default() -> Self {
        Self(Cache::builder().time_to_live(CACHE_TTL).build())
    }
}

impl BuyTokenAccountCache {
    /// The resolution cached under the auction id, or `resolve`'s result
    /// stored under it. Resolvers arriving while one is in flight wait for
    /// its result instead of starting their own, and a failed resolution is
    /// not cached.
    pub async fn resolve<E: Send + Sync + 'static>(
        &self,
        auction_id: Id,
        resolve: impl Future<Output = Result<BuyTokenAccounts, E>>,
    ) -> Result<Arc<BuyTokenAccounts>, Arc<E>> {
        self.0
            .try_get_with(auction_id, async { resolve.await.map(Arc::new) })
            .await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::future};

    fn id(id: i64) -> Id {
        Id::new(id).unwrap()
    }

    fn resolved() -> impl Future<Output = Result<BuyTokenAccounts, ()>> {
        future::ready(Ok(BuyTokenAccounts::default()))
    }

    fn failed() -> impl Future<Output = Result<BuyTokenAccounts, ()>> {
        future::ready(Err(()))
    }

    /// The second engine to solve an auction reads the first one's
    /// resolution: a resolver that can only fail still answers for the cached
    /// id, and a failed resolution never sticks.
    #[tokio::test]
    async fn looks_an_auction_up_once() {
        let cache = BuyTokenAccountCache::default();

        cache.resolve(id(1), resolved()).await.unwrap();
        cache
            .resolve(id(1), failed())
            .await
            .expect("a cached resolution needs no new lookup");

        cache
            .resolve(id(2), failed())
            .await
            .expect_err("another auction is resolved again");
        cache
            .resolve(id(2), resolved())
            .await
            .expect("a failed resolution is not cached");
    }
}
