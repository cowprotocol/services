use {
    crate::cache::Storage,
    ethcontract::futures::future::join_all,
    ethrpc::Web3,
    shared::maintenance::Maintaining,
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub struct AmmTokenBalanceMaintainer {
    storage: Arc<RwLock<Vec<Storage>>>,
    web3: Web3,
}

impl AmmTokenBalanceMaintainer {
    pub fn new(storage: Arc<RwLock<Vec<Storage>>>, web3: Web3) -> Self {
        Self { storage, web3 }
    }
}

#[async_trait::async_trait]
impl Maintaining for AmmTokenBalanceMaintainer {
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        let lock = self.storage.read().await;
        let futures: Vec<_> = lock
            .iter()
            .map(|storage| async move { storage.drop_empty_amms(&self.web3.clone()).await })
            .collect();
        join_all(futures).await;
        Ok(())
    }

    fn name(&self) -> &str {
        "AmmTokenBalanceHandler"
    }
}
