//! Module containing traits for abstracting Web3 operations so components can
//! more easily be tested with mocked versions of these behaviours.

use {
    crate::web3::Web3,
    alloy::{
        primitives::{Address, Bytes},
        providers::Provider,
    },
    anyhow::Result,
    cached::{Cached, SizedCache},
    std::sync::{Arc, Mutex},
    tracing::instrument,
};

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait::async_trait]
pub trait CodeFetching: Send + Sync + 'static {
    /// Fetches the code for the specified address.
    async fn code(&self, address: Address) -> Result<Bytes>;

    /// Fetches the code size at the specified address.
    async fn code_size(&self, address: Address) -> Result<usize>;
}

#[async_trait::async_trait]
impl CodeFetching for Web3 {
    #[instrument(skip_all)]
    async fn code(&self, address: Address) -> Result<Bytes> {
        Ok(self.provider.get_code_at(address).await?)
    }

    #[instrument(skip_all)]
    async fn code_size(&self, address: Address) -> Result<usize> {
        Ok(self.code(address).await?.0.len())
    }
}

pub struct CachedCodeFetcher {
    inner: Arc<dyn CodeFetching>,
    cache: Mutex<SizedCache<Address, Bytes>>,
}

impl CachedCodeFetcher {
    const CACHE_SIZE: usize = 1_000;

    pub fn new(inner: Arc<dyn CodeFetching>) -> Self {
        Self {
            inner,
            cache: Mutex::new(SizedCache::with_size(Self::CACHE_SIZE)),
        }
    }

    async fn cached_code<T, F>(&self, address: Address, handle: F) -> Result<T>
    where
        F: FnOnce(&Bytes) -> T,
    {
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(code) = cache.cache_get(&address) {
                return Ok(handle(code));
            }
        }

        let code = self.inner.code(address).await?;
        let result = handle(&code);
        self.cache.lock().unwrap().cache_set(address, code);
        Ok(result)
    }
}

#[async_trait::async_trait]
impl CodeFetching for CachedCodeFetcher {
    async fn code(&self, address: Address) -> Result<Bytes> {
        self.cached_code(address, |code| code.clone()).await
    }

    async fn code_size(&self, address: Address) -> Result<usize> {
        self.cached_code(address, |code| code.0.len()).await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        mockall::{Sequence, predicate},
    };

    #[tokio::test]
    async fn caches_code() {
        let mut inner = MockCodeFetching::new();
        let mut seq = Sequence::new();
        inner
            .expect_code()
            .times(1)
            .with(predicate::eq(Address::repeat_byte(1)))
            .returning(|_| Ok(Bytes::from(vec![1; 1])))
            .in_sequence(&mut seq);
        inner
            .expect_code()
            .times(1)
            .with(predicate::eq(Address::repeat_byte(2)))
            .returning(|_| Ok(Bytes::from(vec![2; 2])))
            .in_sequence(&mut seq);

        let cached = CachedCodeFetcher::new(Arc::new(inner));

        assert_eq!(
            cached.code(Address::repeat_byte(1)).await.unwrap(),
            Bytes::from_static(&[1])
        );
        assert_eq!(cached.code_size(Address::repeat_byte(1)).await.unwrap(), 1);

        assert_eq!(cached.code_size(Address::repeat_byte(2)).await.unwrap(), 2);
        assert_eq!(
            cached.code(Address::repeat_byte(2)).await.unwrap(),
            Bytes::from_static(&[2; 2])
        );
    }
}
