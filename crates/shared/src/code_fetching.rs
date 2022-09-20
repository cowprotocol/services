//! Module containing traits for abstracting Web3 operations so components can
//! more easily be tested with mocked versions of these behaviours.

use crate::Web3;
use anyhow::Result;
use cached::{Cached, SizedCache};
use std::sync::{Arc, Mutex};
use web3::types::{Bytes, H160};

#[mockall::automock]
#[async_trait::async_trait]
pub trait CodeFetching: Send + Sync + 'static {
    /// Fetches the code size at the specified address.
    async fn code(&self, address: H160) -> Result<Bytes>;

    /// Fetches the code for the specified address.
    async fn code_size(&self, address: H160) -> Result<usize>;
}

#[async_trait::async_trait]
impl CodeFetching for Web3 {
    async fn code(&self, address: H160) -> Result<Bytes> {
        Ok(self.eth().code(address, None).await?)
    }

    async fn code_size(&self, address: H160) -> Result<usize> {
        Ok(self.code(address).await?.0.len())
    }
}

pub struct CachedCodeFetcher {
    inner: Arc<dyn CodeFetching>,
    cache: Mutex<SizedCache<H160, Bytes>>,
}

impl CachedCodeFetcher {
    const CACHE_SIZE: usize = 1_000;

    pub fn new(inner: Arc<dyn CodeFetching>) -> Self {
        Self {
            inner,
            cache: Mutex::new(SizedCache::with_size(Self::CACHE_SIZE)),
        }
    }

    async fn cached_code<T, F>(&self, address: H160, handle: F) -> Result<T>
    where
        F: FnOnce(&Bytes) -> T,
    {
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(code) = cache.cache_get(&address) {
                return Ok(handle(code));
            }
        }

        dbg!(address);
        let code = self.inner.code(address).await?;
        let result = handle(&code);
        self.cache.lock().unwrap().cache_set(address, code);
        Ok(result)
    }
}

#[async_trait::async_trait]
impl CodeFetching for CachedCodeFetcher {
    async fn code(&self, address: H160) -> Result<Bytes> {
        self.cached_code(address, |code| code.clone()).await
    }

    async fn code_size(&self, address: H160) -> Result<usize> {
        self.cached_code(address, |code| code.0.len()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::{predicate, Sequence};

    #[tokio::test]
    async fn caches_code() {
        let mut inner = MockCodeFetching::new();
        let mut seq = Sequence::new();
        inner
            .expect_code()
            .times(1)
            .with(predicate::eq(H160([1; 20])))
            .returning(|_| Ok(Bytes(vec![1; 1])))
            .in_sequence(&mut seq);
        inner
            .expect_code()
            .times(1)
            .with(predicate::eq(H160([2; 20])))
            .returning(|_| Ok(Bytes(vec![2; 2])))
            .in_sequence(&mut seq);

        let cached = CachedCodeFetcher::new(Arc::new(inner));

        assert_eq!(cached.code(H160([1; 20])).await.unwrap().0, [1]);
        assert_eq!(cached.code_size(H160([1; 20])).await.unwrap(), 1);

        assert_eq!(cached.code_size(H160([2; 20])).await.unwrap(), 2);
        assert_eq!(cached.code(H160([2; 20])).await.unwrap().0, [2; 2]);
    }
}
