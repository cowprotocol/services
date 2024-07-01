pub(crate) mod legacy;

use {crate::Amm, anyhow::Result, contracts::CowAmmLegacyHelper};

#[async_trait::async_trait]
pub trait Deployment: Sync + Send {
    /// Returns the AMM deployed in the given Event.
    async fn deployed_amm(&self, helper: &CowAmmLegacyHelper) -> Result<Option<Amm>>;
}
