use {
    crate::database::Postgres,
    anyhow::{Context, Result},
    serde::Serialize,
    shared::sources::balancer_v2::GetRegisteredPools,
    std::{sync::Arc, time::Duration},
};

pub struct RegisteredPoolsStoring<T: Serialize + Send + Sync> {
    pub graph: Arc<dyn GetRegisteredPools<T>>,
    pub db: Postgres,
}

impl<T: Serialize + Send + Sync> RegisteredPoolsStoring<T> {
    pub async fn run_forever(self) -> ! {
        loop {
            match self.update().await {
                Ok(true) => (),
                Ok(false) => tokio::time::sleep(Duration::from_secs(60)).await,
                Err(err) => {
                    tracing::error!(?err, "RegisteredPoolsStoring update task failed");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    /// Update database for latest registered pools.
    ///
    /// Returns whether an update was performed.
    async fn update(&self) -> Result<bool> {
        let pools = self
            .graph
            .get_registered_pools()
            .await
            .context("get_registered_pools")?;

        let json = &serde_json::to_value(&pools)?;

        let mut ex = self.db.0.acquire().await?;
        database::registered_pools::save(&mut ex, json)
            .await
            .context("update liquidity pools")?;

        Ok(true)
    }
}
