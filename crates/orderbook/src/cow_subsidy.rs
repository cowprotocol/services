use std::{sync::Mutex, time::Duration};

use anyhow::Result;
use cached::{Cached, TimedSizedCache};
use contracts::ERC20;
use primitive_types::{H160, U256};

const CACHE_SIZE: usize = 10000;
const CACHE_LIFESPAN: Duration = Duration::from_secs(60 * 60);

/// Check whether a user qualifies for an extra fee subsidy because they own enough cow token.
#[async_trait::async_trait]
pub trait CowSubsidy: Send + Sync + 'static {
    async fn cow_subsidy_factor(&self, user: H160) -> Result<f64>;
}

pub struct FixedCowSubsidy(pub f64);

impl Default for FixedCowSubsidy {
    fn default() -> Self {
        Self(1.0)
    }
}

#[async_trait::async_trait]
impl CowSubsidy for FixedCowSubsidy {
    async fn cow_subsidy_factor(&self, _: H160) -> Result<f64> {
        Ok(self.0)
    }
}

#[derive(Debug, Clone, Copy)]
enum SubsidyTier {
    None,
    Full,
}

pub struct CowSubsidyImpl {
    cow_token: ERC20,
    threshold: U256,
    factor: f64,
    cache: Mutex<TimedSizedCache<H160, SubsidyTier>>,
}

#[async_trait::async_trait]
impl CowSubsidy for CowSubsidyImpl {
    async fn cow_subsidy_factor(&self, user: H160) -> Result<f64> {
        if let Some(tier) = self.cache.lock().unwrap().cache_get(&user).copied() {
            return Ok(self.tier_to_factor(tier));
        }
        let tier = self.subsidy_tier_uncached(user).await?;
        self.cache.lock().unwrap().cache_set(user, tier);
        Ok(self.tier_to_factor(tier))
    }
}

impl CowSubsidyImpl {
    pub fn new(cow_token: ERC20, threshold: U256, factor: f64) -> Self {
        let cache = TimedSizedCache::with_size_and_lifespan_and_refresh(
            CACHE_SIZE,
            CACHE_LIFESPAN.as_secs(),
            false,
        );
        Self {
            cow_token,
            threshold,
            factor,
            cache: Mutex::new(cache),
        }
    }

    async fn subsidy_tier_uncached(&self, user: H160) -> Result<SubsidyTier> {
        let balance = self.cow_token.balance_of(user).call().await?;
        let factor = if balance < self.threshold {
            SubsidyTier::None
        } else {
            SubsidyTier::Full
        };
        Ok(factor)
    }

    fn tier_to_factor(&self, tier: SubsidyTier) -> f64 {
        match tier {
            SubsidyTier::None => 1.0,
            SubsidyTier::Full => self.factor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;
    use shared::Web3;

    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        let transport = shared::transport::create_env_test_transport();
        let web3 = Web3::new(transport);
        let token = H160(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        let token = ERC20::at(&web3, token);
        let subsidy = CowSubsidyImpl::new(token, U256::from_f64_lossy(1e18), 0.5);
        for i in 0..2 {
            let user = H160::from_low_u64_be(i);
            let result = subsidy.cow_subsidy_factor(user).await;
            println!("{:?} {:?}", user, result);
        }
    }
}
