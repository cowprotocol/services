use {
    crate::{cache::Storage, Amm},
    contracts::ERC20,
    ethcontract::futures::future::join_all,
    ethrpc::Web3,
    shared::maintenance::Maintaining,
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub struct EmptyPoolRemoval {
    storage: Arc<RwLock<Vec<Storage>>>,
    web3: Web3,
}

impl EmptyPoolRemoval {
    pub fn new(storage: Arc<RwLock<Vec<Storage>>>, web3: Web3) -> Self {
        Self { storage, web3 }
    }

    /// Checks if the given AMM has a zero token balance.
    async fn check_single(&self, amm: Arc<Amm>) -> bool {
        let amm_address = amm.address();
        let futures = amm.traded_tokens().iter().map(move |token| async move {
            match ERC20::at(&self.web3, *token)
                .balance_of(*amm_address)
                .call()
                .await
            {
                Ok(balance) => balance.is_zero(),
                Err(err) => {
                    tracing::warn!(
                        amm = ?amm_address,
                        ?token,
                        ?err,
                        "failed to check AMM token balance"
                    );
                    false
                }
            }
        });
        join_all(futures).await.into_iter().any(|is_empty| is_empty)
    }
}

#[async_trait::async_trait]
impl Maintaining for EmptyPoolRemoval {
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        let mut amms_to_check = Vec::<Arc<Amm>>::new();
        {
            let lock = self.storage.read().await;
            for storage in lock.iter() {
                amms_to_check.extend(storage.cow_amms().await);
            }
        }
        let futures = amms_to_check.iter().map(|amm| async {
            self.check_single(amm.clone())
                .await
                .then_some(*amm.address())
        });

        let empty_amms: Vec<_> = join_all(futures).await.into_iter().flatten().collect();
        if !empty_amms.is_empty() {
            tracing::debug!(amms = ?empty_amms, "removing AMMs with zero token balance");
            let lock = self.storage.read().await;
            join_all(
                lock.iter()
                    .map(|storage| async { storage.remove_amms(&empty_amms).await }),
            )
            .await;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "EmptyPoolRemoval"
    }
}
