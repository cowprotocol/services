use super::{BadTokenDetecting, TokenQuality};
use anyhow::Result;
use primitive_types::H160;
use std::{collections::HashMap, sync::Mutex};

pub struct CachingDetector {
    inner: Box<dyn BadTokenDetecting>,
    // std mutex is fine because we don't hold lock across await.
    cache: Mutex<HashMap<H160, TokenQuality>>,
}

#[async_trait::async_trait]
impl BadTokenDetecting for CachingDetector {
    async fn detect(&self, token: H160) -> Result<TokenQuality> {
        if let Some(result) = self.get_from_cache(&token) {
            return Ok(result);
        }

        let result = self.inner.detect(token).await?;
        self.insert_into_cache(token, result.clone());
        Ok(result)
    }
}

impl CachingDetector {
    pub fn new(inner: Box<dyn BadTokenDetecting>) -> Self {
        Self {
            inner,
            cache: Default::default(),
        }
    }

    fn get_from_cache(&self, token: &H160) -> Option<TokenQuality> {
        self.cache.lock().unwrap().get(token).cloned()
    }

    fn insert_into_cache(&self, token: H160, quality: TokenQuality) {
        self.cache.lock().unwrap().insert(token, quality);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bad_token::MockBadTokenDetecting;
    use futures::FutureExt;

    #[test]
    fn goes_to_cache() {
        // Would panic if called twice.
        let mut inner = MockBadTokenDetecting::new();
        inner
            .expect_detect()
            .times(1)
            .returning(|_| Ok(TokenQuality::Good));

        let detector = CachingDetector::new(Box::new(inner));

        for _ in 0..2 {
            let result = detector
                .detect(H160::from_low_u64_le(0))
                .now_or_never()
                .unwrap();
            assert!(result.unwrap().is_good());
        }
    }
}
